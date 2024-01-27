use std::{
    cell::RefCell,
    error::Error,
    path::{Path, PathBuf},
    fs, fs::{DirEntry, File},
    io, io::{Cursor, Read, Seek, SeekFrom, Write},
    os::windows::fs::MetadataExt,
    rc::{Rc, Weak},
    time::Instant,
};
use crate::{
    io_toc::{
        ContainerHeader, ContainerHeaderPackage, IoChunkId, IoChunkType4, IoDirectoryIndexEntry, IoFileIndexEntry, 
        IoStringPool, IoStoreTocEntryMeta, IoStoreTocHeaderType3, IoStoreTocCompressedBlockEntry, IoOffsetAndLength
    },
    string::{FString32NoHash, FStringSerializer, FStringSerializerExpectedLength, Hasher, Hasher16}
};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem
};

//
//      --- A ---
//      |   |   |
//      v   v   v
//      B   C   D
pub struct TocDirectory {
    pub name: String,
    //pub os_path: PathBuf, store os_path indepdenently, it's an absolute path that will be different between mods
    //pub parent: Option<Weak<TocDirectory>>,
    pub children: Vec<TocDirectory>,
    pub files: Vec<TocFile>
}

impl TocDirectory {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            children: vec![],
            files: vec![]
        }
    }
}

pub struct TocFile {
    name: String,
    file_size: u64
}
//      A <--------
//      ^    ^    ^
//      |    |    | (refs from child -> parent)
//      v    |    | (owns from parent -> child and in sibling and file linked lists)
//      B -> C -> D
pub struct TocDirectory2 {
    // there's some performance degradation with RefCell since that checks borrowing rules at runtime
    // there's definitely some faster way to handle this, I'll explore that later I just want something that works for now lol
    pub name:           String, // leaf name only (directory name or file name)
    pub parent:         RefCell<Weak<TocDirectory2>>, // weakref to parent for path building for FIoChunkIds
    pub first_child:    RefCell<Option<Rc<TocDirectory2>>>, // first child
    last_child:         RefCell<Weak<TocDirectory2>>, // O(1) insertion on directory add
    pub next_sibling:   RefCell<Option<Rc<TocDirectory2>>>, // next sibling
    pub first_file:     RefCell<Option<Rc<TocFile2>>>, // begin file linked list, owns file children
    last_file:          RefCell<Weak<TocFile2>>, // O(1) insertion on file add
}

impl TocDirectory2 {
    // constructor
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            parent: RefCell::new(Weak::new()),
            first_child: RefCell::new(None),
            last_child: RefCell::new(Weak::new()),
            next_sibling: RefCell::new(None), // root folder has no siblings
            first_file: RefCell::new(None),
            last_file: RefCell::new(Weak::new())
        }
    }
    // convenience function to create reference counted toc directories
    #[inline]
    pub fn new_rc(name: &str) -> Rc<Self> {
        Rc::new(TocDirectory2::new(name))
    }
    // Returns true/false depending on if the target directory contains any child directories
    fn has_children(dir: Rc<TocDirectory2>) -> bool {
        match *dir.first_child.borrow() {
            Some(_) => true,
            None => false
        }
    }
    // Returns true/false depending on if the target directory contains any child files
    fn has_files(dir: Rc<TocDirectory2>) -> bool {
        match *dir.first_file.borrow() {
            Some(_) => true,
            None => false
        }
    }
    // Add a file child into directory that doesn't currently contain any other files
    #[inline]
    fn add_first_file(dir: Rc<TocDirectory2>, file: Rc<TocFile2>) {
        println!("ADD FIRST FILE ONTO {}: NAME {}", dir.name, file.name);
        *dir.first_file.borrow_mut() = Some(Rc::clone(&file));
        *dir.last_file.borrow_mut() = Rc::downgrade(&file);
    }
    // Replace an existing file in the file list. Kick it off the list so it drops on add_or_replace_file's scope
    #[inline]
    fn replace_file(
        dir: Rc<TocDirectory2>, // containing directory
        prev_file: Option<Rc<TocFile2>>, // previous file, which links to replacee (unless it's the *first* file)
        replacee: Rc<TocFile2>, // like the "e" in bladee
        replacer: Rc<TocFile2> // file that'll take the place of replacee in the chain
    ) {
        println!("REPLACE FILE IN {}: SWAP {} WITH {} (TODO)", dir.name, replacee.name, replacer.name);
    }
    // Add a file to the end of the directory's file list, which contains at least 1 existing file
    #[inline]
    fn add_another_file(dir: Rc<TocDirectory2>, file: Rc<TocFile2>) {
        println!("ADD ANOTHER FILE ONTO {}: NAME {}", dir.name, file.name);
        *dir.last_file.borrow().upgrade().unwrap().next.borrow_mut() = Some(Rc::clone(&file)); // own our new child on the end of children linked list
        *dir.last_file.borrow_mut() = Rc::downgrade(&file); // and set the tail to weakref of the new child
    }
    // go through file list to check if the target file already exists, then replace it with our own
    // otherwise, add our file to the end
    pub fn add_or_replace_file(dir: Rc<TocDirectory2>, file: Rc<TocFile2>) {
        match TocDirectory2::has_files(Rc::clone(&dir)) {
            true => { // :adachi_true: - search file linked list
                let mut found = false;
                let mut prev: Option<Rc<TocFile2>> = None;
                let mut curr_file = Rc::clone(dir.first_file.borrow().as_ref().unwrap());
                loop {
                    if curr_file.name == file.name { // we got the file, replace it
                        found = true;
                        break
                    }
                    match Rc::clone(&curr_file).next.borrow().as_ref() { // check if next points to next entry in chain or ends the chain
                        Some(f) => {
                            prev = Some(Rc::clone(&curr_file));
                            curr_file = Rc::clone(&f);
                        },
                        None => { // couldn't find it to replace, add it to the end
                            break // we need to escape this scope to prevent creating mut ref of last_file->next while const ref last_file->next is still valid
                        }
                    }
                }
                if !found {
                    TocDirectory2::add_another_file(Rc::clone(&dir), Rc::clone(&file));
                } else {
                    TocDirectory2::replace_file(
                        Rc::clone(&dir),
                        prev, // prev is only set with second file in list onwards
                        Rc::clone(&curr_file),
                        Rc::clone(&file)
                    );
                }
            },
            false => TocDirectory2::add_first_file(Rc::clone(&dir), Rc::clone(&file))
        }
    }
    // get a child directory from a parent directory if it exists
    // TODO: use a better search method (currently using a linear search)
    pub fn get_child_dir(parent: Rc<TocDirectory2>, exist: &str) -> Option<Rc<TocDirectory2>> {
        match TocDirectory2::has_children(Rc::clone(&parent)) {
            true => {
                let mut curr_dir = Rc::clone(parent.first_child.borrow().as_ref().unwrap());
                let mut result = None;
                loop {
                    if curr_dir.name == exist { // we got our directory
                        result = Some(Rc::clone(&curr_dir));
                        break;
                    }
                    match Rc::clone(&curr_dir).next_sibling.borrow().as_ref() {
                        Some(ip) => curr_dir = Rc::clone(&ip),
                        None => break
                    }
                }
                result
            },
            false => None // has no children, can only not exist
        }
    }
    pub fn add_directory(parent: Rc<TocDirectory2>, child: Rc<TocDirectory2>) {
        *child.parent.borrow_mut() = Rc::downgrade(&parent); // set child node's parent as weak ref of parent
        println!("adding new directory child {}", child.name);
        if !TocDirectory2::has_children(Rc::clone(&parent)) { // if parent has no nodes (if let doesn't work here since scope of &first_child extends to entire statement, overlapping &mut first_child)
            *parent.first_child.borrow_mut() = Some(Rc::clone(&child)); // head and tail set to new child
            *parent.last_child.borrow_mut() = Rc::downgrade(&child);
            return;
        }
        *parent.last_child.borrow().upgrade().unwrap().next_sibling.borrow_mut() = Some(Rc::clone(&child)); // own our new child on the end of children linked list
        *parent.last_child.borrow_mut() = Rc::downgrade(&child); // and set the tail to weakref of the new child
    }
}

#[derive(Debug, PartialEq)]
pub struct TocFile2 {
    next: RefCell<Option<Rc<TocFile2>>>,
    name: String,
    file_size: u64,
    os_file_path: String // needed so we can open it, copy it then write it into partition
}

impl TocFile2 {
    // constructor
    fn new(name: &str, file_size: u64, os_path: &str) -> Self {
        Self {
            next: RefCell::new(None),
            name: String::from(name),
            file_size,
            os_file_path: String::from(os_path)
        }
    }
    // convenience function to create reference counted toc files
    #[inline]
    pub fn new_rc(name: &str, file_size: u64, os_path: &str) -> Rc<Self> {
        Rc::new(TocFile2::new(name, file_size, os_path))
    }
}

pub const SUITABLE_FILE_EXTENSIONS: &'static [&'static str] = ["uasset", "ubulk", "uptnl"].as_slice();
pub const MOUNT_POINT: &'static str = "../../../";

pub fn add_from_folders2(parent: Rc<TocDirectory2>, os_path: &PathBuf) {
    // We've already checked that this path exists in AddFromFolders, so unwrap directly
    // This folder is equivalent to /[ProjectName]/Content, so our mount point will be
    // at least ../../../[ProjectName] (../../../Game/)
    // build an unsorted n-tree of directories and files, preorder traversal
    // higher priority mods should overwrite contents of files, but not directories
    //println!("add_from_folders2: {}", os_path.to_str().unwrap());
    for i in fs::read_dir(os_path).unwrap() {
        match &i {
            Ok(fs_obj) => { // we have our file system object, now determine if it's a directory or folder
                let fs_obj_os_name = fs_obj.file_name(); // this does assume that the object name is valid Unicode
                let name = String::from(fs_obj_os_name.to_str().unwrap()); // if it's not i'll be very surprised
                let file_type = fs_obj.file_type().unwrap();
                if file_type.is_dir() { // new directory. mods can only expand on this
                    let mut inner_path = PathBuf::from(os_path);
                    inner_path.push(&name);
                    match TocDirectory2::get_child_dir(Rc::clone(&parent), &name) {
                        // check through folder regardless since there may be new inner folders in there
                        Some(child_dir) => add_from_folders2(Rc::clone(&child_dir), &inner_path),
                        None => {
                            // this is a new directory, create it and then check inside it
                            let new_dir = TocDirectory2::new_rc(&name);
                            TocDirectory2::add_directory(Rc::clone(&parent), Rc::clone(&new_dir));
                            add_from_folders2(Rc::clone(&new_dir), &inner_path);
                        }
                    }
                } else if file_type.is_file() {
                    // ignore .uexp, that will be combined in build_table_of_contents
                    match PathBuf::from(&name).extension() {
                        Some(ext) => {
                            let ext_str = ext.to_str().unwrap();
                            match SUITABLE_FILE_EXTENSIONS.iter().position(|exist| *exist == ext_str) {
                                Some(_) => {
                                    // it's a matter of either replacing an existing file or adding a new file
                                    // ,,,at least until we start thinking about merging P3RE persona tables (lol)
                                    let new_file = TocFile2::new_rc(&name, fs_obj.metadata().unwrap().file_size(), fs_obj.path().to_str().unwrap());
                                    TocDirectory2::add_or_replace_file(Rc::clone(&parent), Rc::clone(&new_file));
                                }
                                None => println!("WARNING: {} is not a supported file extension for IO store, skipping...", ext_str)
                            }
                        }
                        None => println!("WARNING: File {} contains no file extension, skipping...", &name)
                    }
                } // but Riri, what about symlinks ?????
            },
            Err(e) => println!("ERROR: Could not add the target file/directory. Reason: {}", e.to_string())
        }
    }
}
pub fn print_contents2(dir: Rc<TocDirectory2>, dir_count: &mut i32, file_count: &mut i32) {
    // just for debugging...
    println!("DIR {}: NAME {}", *dir_count, &dir.name);
    *dir_count += 1;
    // get inner directories
    match dir.first_child.borrow().as_ref() {
        Some(inner) => {
            let mut inner_dir = Rc::clone(inner);
            loop {
                print_contents2(Rc::clone(&inner_dir), dir_count, file_count);
                match Rc::clone(&inner_dir).next_sibling.borrow().as_ref() {
                    Some(next) => {
                        inner_dir = Rc::clone(&next);
                    },
                    None => break,
                }
            }
        },
        None => ()
    };
    // get inner files
    if let Some(f) = dir.first_file.borrow().as_ref() {
        let mut inner_file = Rc::clone(f);
        loop {
            println!("FILE {}: NAME {}", *file_count, &inner_file.name);
            *file_count += 1;
            match Rc::clone(&inner_file).next.borrow().as_ref() {
                Some(next) => inner_file = Rc::clone(next),
                None => break
            }
        }
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
        let toc_name_hash = Hasher16::get_cityhash64("UnrealEssentials_P"); // used for container id (is also the last file in partition) (verified)
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
    pub fn flatten_toc_tree_2(&mut self, root: Rc<TocDirectory2>) {
        self.directories = self.flatten_toc_tree_dir(Rc::clone(&root));
    }
    fn flatten_toc_tree_dir(&mut self, node: Rc<TocDirectory2>) -> Vec<IoDirectoryIndexEntry> {
        let mut values = vec![];
        let mut flat_value = IoDirectoryIndexEntry {
            name: self.get_string_index(&node.name),
            first_child: u32::MAX,
            next_sibling: u32::MAX,
            first_file: u32::MAX
        };
        // Iterate through each file
        // When we hit a file, we'll need to make it's chunk hash, length + offset and compression blocks
        // also create the meta with a placeholder zero hash since I haven't checked how that's implemented yet
        // this needs refactoring this is *ugly*
        if TocDirectory2::has_files(Rc::clone(&node)) {
            let mut curr_file = Rc::clone(node.first_file.borrow().as_ref().unwrap());
            loop {
                let mut flat_file = IoFileIndexEntry {
                    name: self.get_string_index(&curr_file.name),
                    next_file: u32::MAX,
                    user_data: self.resolved_files,
                    os_path: curr_file.os_file_path.clone()
                };
                // travel upwards through parents to build path
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
                //flat_file.rel_path = path.clone();

                // Get the appropriate chunk type based on file
                // TODO: move this over to a trait method to account for different functionality between UE4 and UE5
                let chunk_type = match PathBuf::from(&curr_file.name).extension() {
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
                let generated_chunk_id = self.create_chunk_id(&path, chunk_type);
                println!("Created chunk id from {}: {:?}", &path, generated_chunk_id);
                self.chunk_ids.push(generated_chunk_id);
                // other parts are made during serialization
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
        if TocDirectory2::has_children(Rc::clone(&node)) {
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

    pub fn serialize(&mut self, handle: HANDLE, toc_path: &str, part_path: &str) -> Vec<u8> {
        type CV = Cursor<Vec<u8>>;
        type EN = byteorder::NativeEndian;
        //println!("Create TOC for {}, partition at {}", toc_path, part_path);
        let mut toc_storage: CV = Cursor::new(vec![]);
        let mut cas_storage: CV = Cursor::new(vec![]);
        let cas_writer = PartitionSerializer::new(self.compression_block_alignment);
        // Get DirectoryIndexSize = Directory Entries + File Entries + Strings
        let directory_index_bytes = self.directories.len() * std::mem::size_of::<IoDirectoryIndexEntry>();
        let file_index_bytes = self.files.len() * crate::io_toc::IO_FILE_INDEX_ENTRY_SERIALIZED_SIZE;
        let mut string_index_bytes = 0;
        self.strings.iter().for_each(|name| string_index_bytes += FString32NoHash::get_expected_length(name));
        let dir_size = directory_index_bytes as u64 + file_index_bytes as u64 + string_index_bytes + 12; // include dir count, entry count and string count (4 bytes each)
        println!("dir size: {}, file size: {}, strings {}, total {}", directory_index_bytes, file_index_bytes, string_index_bytes, dir_size);
        // Write our partition file
        let mut container_header = ContainerHeader::new(self.toc_name_hash);
        for (i, v) in self.files.iter().enumerate() {
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
                    self.metas.push(IoStoreTocEntryMeta::new(&f)); // PLACEHOLDER

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
                    // TODO: read IO Store asset header if it's an ExportBundleData to read as StoreEntries in container summary
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
        self.metas.push(IoStoreTocEntryMeta::new(&container_header));

        let cursor_finish = cas_storage.position();
        // partition can be written by std::fs - FileEmulationFramework only has the open file handle for toc
        match fs::write(part_path, &cas_storage.into_inner()) {
            Ok(_) => println!("Wrote 0x{:X} bytes into {}", cursor_finish, part_path),
            Err(e) => println!("ERROR: Couldn't write to partition file {} reason: {}", part_path, e.to_string())
        };
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
        /* 
        let result = unsafe {
            FileSystem::WriteFile(
                handle,
                Some(toc_inner.as_slice()),
                None,
                None
            )
        };
        match result {
            Ok(_) => println!("Wrote 0x{:X} bytes into {}", toc_length, toc_path),
            Err(e) => println!("ERROR: Couldn't write to TOC file {} reason: {}", toc_path, e.to_string())
        }
        */
        /* Use NT file handle + Win32 write instead of std::fs
        match fs::write(toc_path, &toc_storage.into_inner()) {
            Ok(_) => println!("Wrote 0x{:X} bytes into {}", toc_length, toc_path),
            Err(e) => println!("ERROR: Couldn't write to file {} reason: {}", toc_path, e.to_string())
        }
        */
    }
}

// TODO: Set the mount point further up in mods where the file structure doesn't diverge at root
// TODO: Pass version param (probably as trait) to customize how TOC is produced depenending on the target version
// TODO: Handle creating multiple partitions (not important but would help make this more feature complete)
pub fn build_table_of_contents2(handle: HANDLE, root: Rc<TocDirectory2>, toc_path: &str, part_path: &str) -> Vec<u8> {
    println!("BUILD TABLE OF CONTENTS FOR UnrealEssentials_P.utoc");
    // flatten our tree into a list by pre-order traversal
    let toc_time = Instant::now();
    let mut flatten_time = 0;
    let mut serialize_time = 0;
    let mut resolver = TocResolver::new(&root.name);
    resolver.flatten_toc_tree_2(Rc::clone(&root));
    flatten_time = toc_time.elapsed().as_micros();
    let toc_stream = resolver.serialize(handle, toc_path, part_path);
    serialize_time = toc_time.elapsed().as_micros() - flatten_time;
    println!("Flatten Time: {} ms", flatten_time as f64 / 1000f64);
    println!("Serialize Time: {} ms", serialize_time as f64 / 1000f64);
    toc_stream

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