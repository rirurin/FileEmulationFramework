use std::{
    cell::RefCell,
    error::Error,
    path::{Path, PathBuf},
    fs, fs::{DirEntry, File},
    io, io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    rc::{Rc, Weak},
    time::Instant,
};
use crate::{
    asset_collector::{MOUNT_POINT, SUITABLE_FILE_EXTENSIONS, ROOT_DIRECTORY, TocDirectory, TocFile},
    io_package::{ContainerHeaderPackage, PackageIoSummaryDeserialize, PackageSummary2},
    io_toc::{
        ContainerHeader, IoChunkId, IoChunkType4, IoDirectoryIndexEntry, IoFileIndexEntry, 
        IoStringPool, IoStoreTocEntryMeta, IoStoreTocHeaderType3, IoStoreTocCompressedBlockEntry, IoOffsetAndLength
    },
    platform::Metadata,
    string::{FString32NoHash, FStringSerializer, FStringSerializerExpectedLength, Hasher, Hasher16}
};

pub const TOC_NAME:     &'static str = "UnrealEssentials_P";
pub const TARGET_TOC:   &'static str = "UnrealEssentials_P.utoc";

pub static mut TOC_STREAM: Vec<u8> = vec![];

pub fn build_table_of_contents(toc_path: &str) -> Option<Vec<u8>> {
    // build TOC here
    let path_check = PathBuf::from(toc_path);
    let file_name = path_check.file_name().unwrap().to_str().unwrap(); // unwrap, this is a file
    // check that we're targeting the correct UTOC
    if file_name == TARGET_TOC {
        println!("call build_toc on dummy toc {}", file_name);
        match unsafe { &ROOT_DIRECTORY } {
            Some(root) => {
                println!("Mod files were loaded for {}", file_name);
                Some(build_table_of_contents_inner(Rc::clone(root), toc_path))
            },
            None => {
                println!("WARNING: No mod files were loaded for {}", file_name);
                None
            }
        }
    } else {
        // Not our target TOC
        None
    }
}

pub struct TocResolver {
    pub directories: Vec<IoDirectoryIndexEntry>,
    pub files: Vec<IoFileIndexEntry>,
    pub strings: Vec<String>, // TODO: try testing with map on larger mods
    pub resolved_directories: u32,
    pub resolved_files: u32,
    pub resolved_strings: u32,
    estimated_malloc_size: usize, // at least header in size
    compression_block_size: u32,
    memory_mapping_alignment: u32,
    compression_block_alignment: u32,
    toc_name_hash: u64,
    pub project_name: String, // name that you gave your UE4 project
    pub chunk_ids: Vec<IoChunkId>,
    pub offsets_and_lengths: Vec<IoOffsetAndLength>,
    pub compression_blocks: Vec<IoStoreTocCompressedBlockEntry>,
    pub metas: Vec<IoStoreTocEntryMeta>,
}

impl TocResolver {
    pub fn new(root_name: &str) -> Self {
        let directories: Vec<IoDirectoryIndexEntry> = vec![]; // The resulting directory list will be serialized as an FIoDirectoryIndexEntry
        let files: Vec<IoFileIndexEntry> = vec![]; // Our file list will be serialized as an FIoFileIndexEntry
        let strings: Vec<String> = vec![]; // Strings will be owned by a string pool where there'll be serialized into an FString array
        // Strings will be serialized as FString32NoHash
        let estimated_malloc_size = std::mem::size_of::<IoStoreTocHeaderType3>(); // number of bytes that we're expecting this to take up - IoStoreTocHeader is 0x90 bytes, so start with that
        let compression_block_size = 0x10000; // default for UE 4.27
        let memory_mapping_alignment = 0x4000; // default for UE 4.27 (isn't saved in toc)
        let compression_block_alignment = 0x800; // default for UE 4.27 (isn't saved in toc)
        // every file is virtually put on an alignment of [compression_block_size] (in reality, they're only aligned to nearest 16 bytes)
        // offset section defines where each file's data starts, while compress blocks section defines each compression block
        let toc_name_hash = Hasher16::get_cityhash64(TOC_NAME); // used for container id (is also the last file in partition) (verified)
        let resolved_directories = 0;
        let resolved_files = 0;
        let resolved_strings = 0;
        let project_name = root_name.to_owned();
        Self { 
            directories, files, strings, resolved_directories, resolved_files, resolved_strings, estimated_malloc_size, 
            compression_block_size, memory_mapping_alignment, compression_block_alignment, toc_name_hash, project_name,
            chunk_ids: vec![],
            offsets_and_lengths: vec![],
            compression_blocks: vec![],
            metas: vec![]
        }
    }
    fn get_string_index(&mut self, name: &str) -> u32 {
        // check that our string is unique, else get the index for that....
        (match self.strings.iter().position(|exist| exist == name) {
            Some(i) => i,
            None => {
                self.strings.push(name.to_string());
                println!("added string {} at {}", name, self.strings.len() - 1);
                self.resolved_strings += 1;
                self.strings.len() - 1
            },
        }) as u32
    }
    #[inline]
    pub fn flatten_toc_tree(&mut self, root: Rc<TocDirectory>) {
        self.directories = self.flatten_toc_tree_dir(Rc::clone(&root));
    }
    // Flatten the tree of directories + files into a list of directories and list of files
    fn flatten_toc_tree_dir(&mut self, node: Rc<TocDirectory>) -> Vec<IoDirectoryIndexEntry> {
        let mut values = vec![];
        let mut flat_value = IoDirectoryIndexEntry {
            name: self.get_string_index(&node.name),
            first_child: u32::MAX,
            next_sibling: u32::MAX,
            first_file: u32::MAX
        };
        // Iterate through each file
        if TocDirectory::has_files(Rc::clone(&node)) {
            let mut curr_file = Rc::clone(node.first_file.borrow().as_ref().unwrap());
            loop {
                let mut flat_file = IoFileIndexEntry {
                    name: self.get_string_index(&curr_file.name),
                    next_file: u32::MAX,
                    user_data: self.resolved_files,
                    file_size: curr_file.file_size,
                    os_path: curr_file.os_file_path.clone(),
                    hash_path: String::new()

                };
                // travel upwards through parents to build path
                // calculate hash after validation so it's easier to remove incorrectly formatted uassets
                let mut path_comps: Vec<String> = vec![];
                let mut curr_parent = Rc::clone(&node);
                loop {
                    path_comps.insert(0, curr_parent.name.to_owned());
                    match Rc::clone(&curr_parent).parent.borrow().upgrade() {
                        Some(ip) => curr_parent = Rc::clone(&ip),
                        None => break
                    }
                }
                let filename_buf = PathBuf::from(&curr_file.name);
                let path = path_comps.join("/") + "/" + filename_buf.file_stem().unwrap().to_str().unwrap();
                println!("{} PATH: {}", &curr_file.name, &path);
                flat_file.hash_path = path;
                // go to next file
                self.resolved_files += 1;
                match Rc::clone(&curr_file).next.borrow().as_ref() {
                    Some(next) => {
                        flat_file.next_file = self.resolved_files;
                        self.files.push(flat_file);
                        curr_file = Rc::clone(next)
                    },
                    None => {
                        self.files.push(flat_file);
                        break
                    }
                }
            }
        }
        // Iterate through inner directories
        self.resolved_directories += 1;
        println!("flatten(): {}, id {}", &node.name, self.resolved_directories - 1);
        if TocDirectory::has_children(Rc::clone(&node)) {
            flat_value.first_child = self.resolved_directories;
            values.push(flat_value);
            let mut curr_child = Rc::clone(node.first_child.borrow().as_ref().unwrap());
            loop {
                let mut children = self.flatten_toc_tree_dir(Rc::clone(&curr_child));
                match Rc::clone(&curr_child).next_sibling.borrow().as_ref() { // get the next child (if they exist)
                    Some(next) => {
                        children[0].next_sibling = self.resolved_directories;
                        values.extend(children);
                        curr_child = Rc::clone(next);
                    },
                    None => {
                        values.extend(children);
                        break
                    }
                }
            }
        } else {
            values.push(flat_value);
        }
        values
    }
    pub fn create_chunk_id(&self, file_path: &str, chunk_type: IoChunkType4) -> IoChunkId {
        // replace [BaseDirectory]/Content with /Game/
        let path_to_replace = self.project_name.clone() + "/Content";
        if let Some((_, suffix)) = file_path.to_owned().split_once(&path_to_replace) {
            let path_to_hash = String::from("/Game") + suffix;
            IoChunkId::new(&path_to_hash, chunk_type)
        } else {
            panic!("Path \"{}\" is missing root containing project name + content. Path components were not handled properly", file_path);
        }
    }

    fn get_file_hash(&self, curr_file: &IoFileIndexEntry) -> IoChunkId {
        let chunk_type = match PathBuf::from(&curr_file.os_path).extension() {
            Some(ext) => {
                match SUITABLE_FILE_EXTENSIONS.iter().position(|exist| *exist == ext) {
                    Some(i) => {
                        match i {
                            0 => IoChunkType4::ExportBundleData, //.uasset
                            1 => IoChunkType4::BulkData, // .ubulk
                            2 => IoChunkType4::OptionalBulkData, // .uptnl
                            _ => panic!("ERROR: Did not get a supported file extension. This should've been handled earlier")
                        }
                    },
                    None => panic!("ERROR: Did not get a supported file extension. This should've been handled earlier")
                }
            },
            None => panic!("ERROR: Did not get a file extension. This should've been caught earlier)")
        };
        self.create_chunk_id(&curr_file.hash_path, chunk_type)
    }

    pub const FILE_SUMMARY_READER_ALLOC: usize = 0x2000;

    pub fn serialize<
        TSummary: PackageIoSummaryDeserialize
    >(
        &mut self, 
        profiler: &mut TocBuilderProfiler, 
        toc_path: &str
    ) -> Vec<u8> {
        type CV = Cursor<Vec<u8>>;
        type EN = byteorder::NativeEndian;
        //println!("Create TOC for {}, partition at {}", toc_path, part_path);
        let mut toc_storage: CV = Cursor::new(vec![]);
        let mut cas_storage: CV = Cursor::new(vec![]);
        let cas_writer = PartitionSerializer::new(self.compression_block_alignment);
        // Get DirectoryIndexSize = Directory Entries + File Entries + Strings
        // TODO: move this after file serialization (some files may fail the test)
        let directory_index_bytes = self.directories.len() * std::mem::size_of::<IoDirectoryIndexEntry>();
        let file_index_bytes = self.files.len() * crate::io_toc::IO_FILE_INDEX_ENTRY_SERIALIZED_SIZE;
        let mut string_index_bytes = 0;
        self.strings.iter().for_each(|name| string_index_bytes += FString32NoHash::get_expected_length(name));
        let dir_size = directory_index_bytes as u64 + file_index_bytes as u64 + string_index_bytes + 12; // include dir count, entry count and string count (4 bytes each)
        //println!("dir size: {}, file size: {}, strings {}, total {}", directory_index_bytes, file_index_bytes, string_index_bytes, dir_size);
        // Write our partition file
        let mut container_header = ContainerHeader::new(self.toc_name_hash);
        let mut file_index = 0;
        loop {
            let curr_file = &self.files[file_index];
            match File::open(&curr_file.os_path) {
                Ok(file) => {

                },
                Err(e) => {
                    profiler.failed_to_read.push((&curr_file.os_path).to_owned());
                    profiler.failed_to_read_size += curr_file.file_size;
                    let removed_element = self.files.remove(file_index);
                    // TODO: handle a situation where it's referenced in the directory index as the first file
                    // 
                    continue; // Go to next entry, which will have the same index as the erroneous entry
                }
            }
            file_index = if file_index < self.files.len() - 1 { file_index + 1 } else { break };
        }
        for (i, v) in self.files.iter().enumerate() {
            // we already have all the information we need from the file size in metadata and file name to create all other components of TOC
            // TODO: don't serialize entire stream - instead pass the file path for each asset as a list and build a Multistream in C# side
            // You only need to read as far as GraphDataOffset + GraphDataSize since that's the end of package summaries
            match File::open(&v.os_path) {
                Ok(file) => {
                    // default value can be adjusted, as long as 99.9% of files can be read in one (1) syscall
                    let mut file_reader = BufReader::with_capacity(Self::FILE_SUMMARY_READER_ALLOC, file);
                    if !crate::io_package::is_valid_asset_type::<BufReader<File>, EN>(&mut file_reader) {
                        // not a valid type, remove from list
                        //self.files.remove(i); // rewrite loop to allow mutating
                        continue;
                    }
                    // create the hash for the new file
                    let generated_chunk_id = self.get_file_hash(v);
                    println!("Created chunk id from {}: {:?}", &v.hash_path, generated_chunk_id);
                    self.chunk_ids.push(generated_chunk_id); // push once we're sure that the file's valid
                },
                // TODO: fail gracefully (when using panic Reloaded just hangs)
                Err(e) => panic!("ERROR: Could not read file {}: reason {}", &v.os_path, e.to_string())
            }
            match fs::read(&v.os_path) {
                Ok(f) => {
                    // Generate FIoOffsetAndLength
                    // TODO: Use u64, then check that base_length is smaller than 0xFF FF FF FF FF
                    let curr_file = &self.files[i];
                    let file_offset = self.compression_blocks.len() as u64 * self.compression_block_size as u64;
                    let file_length = f.len() as u64;
                    let generated_offset_length = IoOffsetAndLength::new(file_offset, file_length);
                    println!("Created offset and length for {}: 0x{:X}, 0x{:X}", &curr_file.name, file_offset, file_length);
                    self.offsets_and_lengths.push(generated_offset_length);
                    // Generate compression blocks
                    let compression_block_count = (file_length / self.compression_block_size as u64) + 1; // need at least 1 compression block
                    let mut size_remaining = file_length as u32;
                    for i in 0..compression_block_count {
                        let cmp_size = if size_remaining > self.compression_block_size {self.compression_block_size} else {size_remaining}; // cmp_size = decmp_size
                        let offset = cas_storage.position() + self.compression_block_size as u64 * i;
                        let new_cmp_block = IoStoreTocCompressedBlockEntry::new(offset, cmp_size);
                        //println!("{:?}", new_cmp_block);
                        self.compression_blocks.push(new_cmp_block);
                        if size_remaining > self.compression_block_size {size_remaining -= self.compression_block_size}; // rust panics on overflow by default
                    }
                    // Generate meta
                    // meta is a SHA1 hash of the file's contents
                    self.metas.push(IoStoreTocEntryMeta::new_empty());

                    // Generate container header package entry
                    let mut f_cursor = Cursor::new(f); // move stream to cursor for from_header_package
                    if self.chunk_ids[i].get_type() == IoChunkType4::ExportBundleData {
                        container_header.packages.push(ContainerHeaderPackage::from_header_package::<CV, EN>(
                            &mut f_cursor, 
                            self.chunk_ids[i].get_raw_hash(),
                            file_length)
                        );
                    }
                    let f = f_cursor.into_inner(); // move stream back into f
                    PartitionSerializer::to_buffer::<CV, EN>(&f, &mut cas_storage);
                    cas_writer.to_buffer_alignment::<CV, EN>(&mut cas_storage);
                    println!("new cursor position: 0x{:X}", cas_storage.position());
                },
                Err(e) => panic!("ERROR: Could not read file {}: reason {}", &v.os_path, e.to_string())
            }
        }
        let container_position = cas_storage.position();
        let container_header = container_header.to_buffer::<CV, EN>(&mut cas_storage).unwrap(); // write our container header in the buffer
        self.chunk_ids.push(IoChunkId::new_from_hash(self.toc_name_hash, IoChunkType4::ContainerHeader)); // header chunk id
        let header_offset = self.compression_blocks.len() as u64 * self.compression_block_size as u64; 
        self.offsets_and_lengths.push(IoOffsetAndLength::new(header_offset, container_header.len() as u64)); // header offset + length
        // header compress blocks (use single block for now, make sure to support multiple blocks later)
        self.compression_blocks.push(IoStoreTocCompressedBlockEntry::new(container_position, container_header.len() as u32));
        self.metas.push(IoStoreTocEntryMeta::new_empty());

        let cursor_finish = cas_storage.position();
        // Don't write our ucas here. Instead, pass a series of 
        /* 
        match fs::write(part_path, &cas_storage.into_inner()) {
            Ok(_) => println!("Wrote 0x{:X} bytes into {}", cursor_finish, part_path),
            Err(e) => println!("ERROR: Couldn't write to partition file {} reason: {}", part_path, e.to_string())
        };
        */
        // Write our TOC
        let toc_header = IoStoreTocHeaderType3::new(
            self.toc_name_hash, 
            self.files.len() as u32 + 1, // + 1 for container header
            self.compression_blocks.len() as u32,
            self.compression_block_size,
            dir_size as u32
        );
        // FIoStoreTocHeader
        toc_header.to_buffer::                          <CV, EN>(&mut toc_storage).unwrap(); // FIoStoreTocHeader
        IoChunkId::list_to_buffer::                     <CV, EN>(&self.chunk_ids, &mut toc_storage).unwrap(); // FIoChunkId
        IoOffsetAndLength::list_to_buffer::             <CV, EN>(&self.offsets_and_lengths, &mut toc_storage).unwrap(); // FIoOffsetAndLength
        IoStoreTocCompressedBlockEntry::list_to_buffer::<CV, EN>(&self.compression_blocks, &mut toc_storage).unwrap(); // FIoStoreTocCompressedBlockEntry
        FString32NoHash::to_buffer::                    <CV, EN>(MOUNT_POINT, &mut toc_storage).unwrap(); // Mount Point
        IoDirectoryIndexEntry::list_to_buffer::         <CV, EN>(&self.directories, &mut toc_storage).unwrap(); // FIoDirectoryIndexEntry
        IoFileIndexEntry::list_to_buffer::              <CV, EN>(&self.files, &mut toc_storage).unwrap(); // FIoFileIndexEntry
        IoStringPool::list_to_buffer::                  <CV, EN>(&self.strings, &mut toc_storage).unwrap(); // FIoStringIndexEntry
        IoStoreTocEntryMeta::list_to_buffer::           <CV, EN>(&self.metas, &mut toc_storage).unwrap(); // FIoStoreTocEntryMeta
        let toc_length = toc_storage.position();
        // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/fn.WriteFile.html
        // TODO: figure out how to hook this up to FileEmulationFramework properly
        // This won't currently work when run through Reloaded, but does work with toc-builder-test
        let toc_inner = toc_storage.into_inner();
        toc_inner
    }
}

// TODO: Set the mount point further up in mods where the file structure doesn't diverge at root
// TODO: Pass version param (probably as trait) to customize how TOC is produced depenending on the target version
// TODO: Support UE5 (sometime soon)
pub fn build_table_of_contents_inner(root: Rc<TocDirectory>, toc_path: &str) -> Vec<u8> {
    println!("BUILD TABLE OF CONTENTS FOR {}", TARGET_TOC);
    // flatten our tree into a list by pre-order traversal
    let mut profiler = TocBuilderProfiler::new();
    let mut resolver = TocResolver::new(&root.name);
    resolver.flatten_toc_tree(Rc::clone(&root));
    profiler.set_flatten_time();
    let toc_stream = resolver.serialize::<PackageSummary2>(&mut profiler, toc_path);
    profiler.set_serialize_time();
    profiler.display_results();
    toc_stream
}

#[repr(C)]
pub struct PartitionBlock {
    path: *const u8,
    start: u64,
    length: u64,
    gap: *const u8
}


pub enum PartitionStatus {
    Successful, // sucessfully got block data
    IncorrectFile, // not our target file
    CalledBeforeToc, // what
}

// Keep in sync with definition in UtocEmulator.cs
impl From<PartitionStatus> for u32 {
    fn from(value: PartitionStatus) -> u32 {
        match value {
            PartitionStatus::Successful => 0,
            PartitionStatus::IncorrectFile => 1,
            PartitionStatus::CalledBeforeToc => 2
        }
    }
}

pub fn get_virtual_partition(cas_path: &str) -> Vec<PartitionBlock> {
    // build virtual CAS here
    let path_check = PathBuf::from(cas_path);
    let file_name = path_check.file_name().unwrap().to_str().unwrap();
    // check that we're targeting the correct UCAS
    vec![]
}

// From string.rs
// Will refactor later
pub trait PartitionSerializerFile {
    fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(file: &Vec<u8>, writer: &mut W) -> Result<(), Box<dyn Error>>;
}

pub trait PartitionSerializerAlign {
    fn get_block_alignment(&self) -> u64;
    fn to_buffer_alignment<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W);
}

pub struct PartitionSerializer(u64);

impl PartitionSerializer {
    pub fn new(block_align: u32) -> Self { // block align stored in TocResolver
        Self(block_align as u64)
    }
}

impl PartitionSerializerFile for PartitionSerializer {
    fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(file: &Vec<u8>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_all(file)?;
        Ok(())
    }
}
impl PartitionSerializerAlign for PartitionSerializer {
    fn get_block_alignment(&self) -> u64 {
        self.0
    }
    fn to_buffer_alignment<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) {
        let align = writer.stream_position().unwrap() % self.get_block_alignment();
        if align == 0 {
            return;
        }
        let diff = self.get_block_alignment() - align;
        writer.seek(SeekFrom::Current(diff as i64));
    }
}

pub struct TocBuilderProfiler {
    // All file sizes are in bytes
    successful_files: u64,
    successful_files_size: u64,
    incorrect_asset_format: Vec<String>, // list of offending files, print out to console
    incorrect_asset_format_size: u64,
    failed_to_read: Vec<String>,
    failed_to_read_size: u64,
    container_header_hash: u64,
    compression_block_count: u64,
    mount_point: String,
    directory_index_size: u64,
    file_index_size: u64,
    string_index_size: u64,
    generated_meta_hashes: bool,
    start_time: Instant,
    time_to_flatten: u128,
    time_to_serialize: u128
}

impl TocBuilderProfiler {
    fn new() -> Self {
        Self {
            successful_files: 0,
            successful_files_size: 0,
            incorrect_asset_format: vec![],
            incorrect_asset_format_size: 0,
            failed_to_read: vec![],
            failed_to_read_size: 0,
            container_header_hash: 0,
            compression_block_count: 0,
            mount_point: String::new(),
            directory_index_size: 0,
            file_index_size: 0,
            string_index_size: 0,
            generated_meta_hashes: false,
            start_time: Instant::now(),
            time_to_flatten: 0,
            time_to_serialize: 0
        }
    }

    fn set_flatten_time(&mut self) {
        self.time_to_flatten = self.start_time.elapsed().as_micros();
    }
    fn set_serialize_time(&mut self) {
        self.time_to_serialize = self.start_time.elapsed().as_micros();
    }
    fn display_results(&self) {
        // TODO: Advanced display results
        println!("Flatten Time: {} ms", self.time_to_flatten as f64 / 1000f64);
        println!("Serialize Time: {} ms", self.time_to_serialize as f64 / 1000f64);
    }
}

// TODO for today:
// - Handle validating asset files in add_from_folders so that none of the flattening/serialization code needs to handle it and make things complicated (that's where the tree's constructed, keep all that related code there)
// - Read the headers for each valid asset to get data to build container file store entry, then get the real offset and length and save that to a struct that will be copied over to the C# side to make a Multistream from
// - Don't bother reading anything more than what's needed for export bundles, and don't read at all for bulk data (2 syscalls needed for ExportBundleData, 0 syscalls needed for BulkData/OptionalBulkData)
// - Create a detailed summary for tree building and TOC creation, including any failed files and time taken to do every operation
// - Fix file replacing (depending on how speed goes, TocDirectory may also be modified to use a map for files)
// - Check to see if loose pak files and UTOC can be used at the same time, and if so, implement DeathChaos's patches to loose load files in PAKs into Unreal Essentials
// - Support 4.25 to 4.27 (in theory)
// - UE5 and cooked package support will come later (end of February perhaps?)