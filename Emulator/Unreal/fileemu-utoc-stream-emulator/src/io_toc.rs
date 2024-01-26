use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt};
use crate::{
    io_package::FGraphPackage,
    string::{FString32NoHash, FStringSerializer, Hasher, Hasher16},
    toc_factory::{PartitionSerializer, PartitionSerializerAlign}
};
use sha1::{Sha1, Digest};
use std::{
    error::Error,
    io::{Cursor, Read, Seek, SeekFrom, Write}
};

pub type IoContainerId = u64; // TODO: ContainerID is a UID as a CityHash64 of the container name
                              // represent that with a distinct CityHashID type
pub type GUID = u128;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(u8)]
#[allow(dead_code)]
// One byte sized enum
// https://doc.rust-lang.org/nomicon/other-reprs.html#repru-repri
pub enum IoStoreTocVersion {
    Invalid = 0,
    Initial, 
    DirectoryIndex, // added in UE 4.25+/4.26 (appears in Scarlet Nexus)
    PartitionSize, // added in UE 4.27
    PerfectHash, // added in UE 5.0
    PerfectHashWithOverflow, // also added in UE 5.0
    //LatestPlusOne
}

impl From<IoStoreTocVersion> for u8 {
    fn from(val: IoStoreTocVersion) -> u8 {
        match val {
            IoStoreTocVersion::Invalid => 0,
            IoStoreTocVersion::Initial => 1,
            IoStoreTocVersion::DirectoryIndex => 2,
            IoStoreTocVersion::PartitionSize => 3,
            IoStoreTocVersion::PerfectHash => 4,
            IoStoreTocVersion::PerfectHashWithOverflow => 5,
        }
    }
}

impl From<u8> for IoStoreTocVersion {
    fn from(val: u8) -> IoStoreTocVersion {
        match val {
            1 => IoStoreTocVersion::Initial,
            2 => IoStoreTocVersion::DirectoryIndex,
            3 => IoStoreTocVersion::PartitionSize,
            4 => IoStoreTocVersion::PerfectHash,
            5 => IoStoreTocVersion::PerfectHashWithOverflow,
            _ => panic!("ERROR: Cannot create an IoStoreTocVersion from value {}", val)
        }
    }
}

bitflags! {
    struct IoContainerFlags : u8 {
        const NoFlags = 0;
        const Compressed = 1 << 0;
        const Encrypted = 1 << 1;
        const Signed = 1 << 2;
        const Indexed = 1 << 3;
        const OnDemand = 1 << 4; // added in UE 5.3 (this flag sounds scary)
    }
}

// IO STORE HEADER

pub const IO_STORE_TOC_MAGIC: &[u8] = b"-==--==--==--==-"; // const stored as static string slice
                                                           // since std::convert::TryInto is not
                                                           // const
pub const IO_STORE_TOC_MAGIC2: [u8; 0x10] = *b"-==--==--==--==-";

pub enum IoStoreToc {
    Initial(IoStoreTocHeaderType1),
    DirectoryIndex(IoStoreTocHeaderType1),
    PartitionSize(IoStoreTocHeaderType1),
    PerfectHash(IoStoreTocHeaderType1)
}

impl IoStoreToc {
    pub fn new<N: AsRef<str>>(ver: IoStoreTocVersion, name: N, entries: u32)  -> IoStoreToc {
        match ver {
            IoStoreTocVersion::Initial => IoStoreToc::Initial(IoStoreTocHeaderType1::new(name, entries)),
            IoStoreTocVersion::DirectoryIndex => IoStoreToc::DirectoryIndex(IoStoreTocHeaderType1::new(name, entries)),
            IoStoreTocVersion::PartitionSize => IoStoreToc::PartitionSize(IoStoreTocHeaderType1::new(name, entries)),
            IoStoreTocVersion::PerfectHash | 
            IoStoreTocVersion::PerfectHashWithOverflow => IoStoreToc::PerfectHash(IoStoreTocHeaderType1::new(name, entries)),
            _ => panic!("Invalid TOC store type"),
        }
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[repr(C)]
pub struct IoStoreTocHeaderType1 { // Unreal Engine 4.25 (size: 0x80) (unverified)
    toc_magic: [u8; 0x10],
    toc_header_size: u32,
    toc_entry_count: u32,
    toc_entry_size: u32, // for sanity checking
    toc_pad: [u32; 25]
}

impl IoStoreTocHeaderType1 {
    fn new<N: AsRef<str>>(name: N, entries: u32) -> IoStoreTocHeaderType1 {
        let toc_magic: [u8; 0x10] = IO_STORE_TOC_MAGIC.try_into().unwrap();
        let toc_header_size = std::mem::size_of::<IoStoreTocHeaderType1>() as u32;
        let toc_entry_count = entries;
        let toc_entry_size = 0;
        let toc_pad = [0; 25];
        IoStoreTocHeaderType1 {
            toc_magic,
            toc_header_size,
            toc_entry_count,
            toc_entry_size,
            toc_pad
        }
    }
}

#[repr(C)]
pub struct IoStoreTocHeaderType2 { // Unreal Engine 4.25+ (Scarlet Nexus), 4.26 
    toc_magic: [u8; 0x10],
    version: IoStoreTocVersion,
    toc_header_size: u32,
    toc_entry_count: u32,
    toc_compressed_block_entry_count: u32,
    toc_compressed_block_entry_size: u32, // for sanity checking
    compression_method_name_count: u32,
    compression_method_name_length: u32,
    compression_block_size: u32,
    directory_index_size: u32,
    container_id: IoContainerId, // cityhash of pak name (e.g "pakchunk0" - b9f66c62c549f00c)                       
    encryption_key_guid: GUID,
    container_flags: IoContainerFlags,
    reserved: [u32; 15]
}

#[repr(C)]
pub struct IoStoreTocHeaderType3 { // Unreal Engine 4.27 (size: 0x90)
    toc_magic: [u8; 0x10],
    version: IoStoreTocVersion,
    toc_header_size: u32,
    toc_entry_count: u32,
    toc_compressed_block_entry_count: u32,
    toc_compressed_block_entry_size: u32, // for sanity checking
    compression_method_name_count: u32,
    compression_method_name_length: u32,
    compression_block_size: u32,
    directory_index_size: u32,
    partition_count: u32,
    container_id: IoContainerId, 
    encryption_key_guid: GUID,
    container_flags: IoContainerFlags,
    partition_size: u64,
    reserved: [u64; 6]
}

impl IoStoreTocHeaderType3 {
    pub fn new(container_id: u64, entries: u32, compressed_blocks: u32, compression_block_size: u32, dir_index_size: u32) -> Self {
        Self {
            toc_magic: IO_STORE_TOC_MAGIC2,
            version: IoStoreTocVersion::PartitionSize,
            toc_header_size: std::mem::size_of::<IoStoreTocHeaderType3>() as u32,
            toc_entry_count: entries,
            toc_compressed_block_entry_count: compressed_blocks,
            toc_compressed_block_entry_size: std::mem::size_of::<IoStoreTocCompressedBlockEntry>() as u32, // for sanity checking
            compression_method_name_count: 0,
            compression_method_name_length: 0,
            compression_block_size,
            directory_index_size: dir_index_size,
            partition_count: 1,
            container_id,
            encryption_key_guid: 0,
            container_flags: IoContainerFlags::Indexed,
            partition_size: u64::MAX,
            reserved: [0; 6]
        }
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_all(self.toc_magic.as_slice())?; // 0x0
        writer.write_u8(self.version.into())?;
        writer.write_u24::<E>(0)?; // padding
        writer.write_u32::<E>(self.toc_header_size);
        writer.write_u32::<E>(self.toc_entry_count);
        writer.write_u32::<E>(self.toc_compressed_block_entry_count);
        writer.write_u32::<E>(self.toc_compressed_block_entry_size);
        writer.write_u32::<E>(self.compression_method_name_count);
        writer.write_u32::<E>(self.compression_method_name_length);
        writer.write_u32::<E>(self.compression_block_size);
        writer.write_u32::<E>(self.directory_index_size);
        writer.write_u32::<E>(self.partition_count);
        writer.write_u64::<E>(self.container_id);
        writer.write_u128::<E>(self.encryption_key_guid);
        writer.write_u8(self.container_flags.bits());
        writer.write_u24::<E>(0)?; // padding
        writer.write_u32::<E>(0)?; // padding
        writer.write_u64::<E>(self.partition_size)?;
        for _ in 0..6 {
            writer.write_u64::<E>(0)?; // padding
        }
        Ok(())
    }
}

#[repr(C)]
pub struct IoStoreTocHeaderType4 { // Unreal Engine 5.0+ (size: 0x90)
    toc_magic: [u8; 0x10],
    version: IoStoreTocVersion,
    toc_header_size: u32,
    toc_entry_count: u32,
    toc_compressed_block_entry_count: u32,
    toc_compressed_block_entry_size: u32, // for sanity checking
    compression_method_name_count: u32,
    compression_method_name_length: u32,
    compression_block_size: u32,
    directory_index_size: u32,
    partition_count: u32,
    container_id: IoContainerId, 
    encryption_key_guid: GUID,
    container_flags: IoContainerFlags,
    toc_chunks_perfect_hash_seeds_count: u32,
    partition_size: u64,
    toc_chunks_without_perfect_hash_count: u32,
    reserved: [u32; 11]
}

// IO CHUNK ID
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
#[allow(dead_code)]
pub enum IoChunkType4 {     
    Invalid = 0,
    InstallManifest,
    ExportBundleData,
    BulkData,
    OptionalBulkData,
    MemoryMappedBulkData,
    LoaderGlobalMeta,
    LoaderInitialLoadMeta,
    LoaderGlobalNames,
    LoaderGlobalNameHashes,
    ContainerHeader // added in UE 4.25+/4.26
}

impl From<u8> for IoChunkType4 {
    fn from(value: u8) -> Self {
        match value {
            1 => IoChunkType4::InstallManifest,
            2 => IoChunkType4::ExportBundleData,
            3 => IoChunkType4::BulkData,
            4 => IoChunkType4::OptionalBulkData,
            5 => IoChunkType4::MemoryMappedBulkData,
            6 => IoChunkType4::LoaderGlobalMeta,
            7 => IoChunkType4::LoaderInitialLoadMeta,
            8 => IoChunkType4::LoaderGlobalNames,
            9 => IoChunkType4::LoaderGlobalNameHashes,
            10 => IoChunkType4::ContainerHeader,
            _ => panic!("Invalid type {} for IoChunkType4", value)
        }
    }
}

impl From<IoChunkType4> for u8 {
    fn from(value: IoChunkType4) -> Self {
        match value {
            IoChunkType4::Invalid => 0,
            IoChunkType4::InstallManifest => 1,
            IoChunkType4::ExportBundleData => 2,
            IoChunkType4::BulkData => 3,
            IoChunkType4::OptionalBulkData => 4,
            IoChunkType4::MemoryMappedBulkData => 5,
            IoChunkType4::LoaderGlobalMeta => 6,
            IoChunkType4::LoaderInitialLoadMeta => 7,
            IoChunkType4::LoaderGlobalNames => 8,
            IoChunkType4::LoaderGlobalNameHashes => 9,
            IoChunkType4::ContainerHeader => 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
#[allow(dead_code)]
pub enum IoChunkType5 {
    Invalid = 0,
    ExportBundleData,
    BulkData,
    OptionalBulkData,
    MemoryMappedBulkData,
    ScriptObjects,
    ContainerHeader,
    ExternalFile,
    ShaderCodeLibrary,
    ShaderCode,
    PackageStoreEntry,
    DerivedData,
    EditorDerivedData,
    PackageResource // added in UE 5.2
}

impl From<u8> for IoChunkType5 {
    fn from(value: u8) -> Self {
        match value {
            1 => IoChunkType5::ExportBundleData,
            2 => IoChunkType5::BulkData,
            3 => IoChunkType5::OptionalBulkData,
            4 => IoChunkType5::MemoryMappedBulkData,
            5 => IoChunkType5::ScriptObjects,
            6 => IoChunkType5::ContainerHeader,
            7 => IoChunkType5::ExternalFile,
            8 => IoChunkType5::ShaderCodeLibrary,
            9 => IoChunkType5::ShaderCode,
            10 => IoChunkType5::PackageStoreEntry,
            11 => IoChunkType5::DerivedData,
            12 => IoChunkType5::EditorDerivedData,
            13 => IoChunkType5::PackageResource,
            _ => panic!("Invalid type {} for IoChunkType4", value)
        }
    }
}

impl From<IoChunkType5> for u8 {
    fn from(value: IoChunkType5) -> Self {
        match value {
            IoChunkType5::Invalid => 0,
            IoChunkType5::ExportBundleData => 1,
            IoChunkType5::BulkData => 2,
            IoChunkType5::OptionalBulkData => 3,
            IoChunkType5::MemoryMappedBulkData => 4,
            IoChunkType5::ScriptObjects => 5,
            IoChunkType5::ContainerHeader => 6,
            IoChunkType5::ExternalFile => 7,
            IoChunkType5::ShaderCodeLibrary => 8,
            IoChunkType5::ShaderCode => 9,
            IoChunkType5::PackageStoreEntry => 10,
            IoChunkType5::DerivedData => 11,
            IoChunkType5::EditorDerivedData => 12,
            IoChunkType5::PackageResource => 13,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(C/* , align(4)*/)] // This is the same across all versions of IO Store UE that i'm aware of
pub struct IoChunkId {
    //id: [u8; 0xc]
    hash: u64,
    index: u16,
    obj_type: IoChunkType4
}

impl IoChunkId {
    pub fn new(path: &str, chunk_type: IoChunkType4) -> Self {
        let hash = Hasher16::get_cityhash64(path); // ChunkId
        let index = 0;
        let pad = 0;
        let obj_type = chunk_type;
        Self { hash, index, obj_type }
    }
    #[inline]
    pub fn new_from_hash(hash: u64, obj_type: IoChunkType4) -> Self {
        Self { hash, index: 0, obj_type }
    }
    // TODO: split to_buffer off as a trait method
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u64::<E>(self.hash)?; // 0x0
        writer.write_u16::<E>(self.index)?; // 0x8
        writer.write_u8(0)?; // 0xa: padding
        writer.write_u8(self.obj_type.into())?; // 0xb
        Ok(())
    }
    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoChunkId>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
    pub fn get_raw_hash(&self) -> u64 {
        self.hash
    }
    pub fn get_type(&self) -> IoChunkType4 {
        self.obj_type
    }
} 

// IO OFFSET + LENGTH
#[derive(Debug)]
#[repr(C)]
pub struct IoOffsetAndLength {
    data: [u8; 0xa]
}

pub const IO_OFFSET_LENGTH_MAX: u64 = 0xFFFFFFFFFF; // 5 bytes (~1.1 TB)
pub const IO_COMPRESSED_BLOCK_LENGTH_MAX: u32 = 0xFFFFFF; // 3 bytes (16.7 MB)

impl IoOffsetAndLength {
    // TODO: proper error handling for offset/length values above IO_OFFSET_LENGTH_MAX
    pub fn new(offset: u64, length: u64) -> Self {
        type ByteBlock = Cursor<[u8; 0xa]>;
        let mut byte_builder = Cursor::new([0; 0xa]);
        let offset_comp = &offset.to_be_bytes()[3..8]; // 0x0
        byte_builder.write_all(offset_comp);
        let length_comp = &length.to_be_bytes()[3..8]; // 0x5
        byte_builder.write_all(length_comp);
        Self {data: byte_builder.into_inner()}
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_all(self.data.as_slice())?;
        Ok(())
    }
    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoOffsetAndLength>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

// (UE 5 ONLY) Perfect Hash

// IO Compression Blocks
#[derive(Debug)]
#[repr(C)]
pub struct IoStoreTocCompressedBlockEntry {
    data: [u8; 0xc] // 5 bytes offset, 3 bytes for size/uncompressed size, 1 byte for compression
                    // method
}

impl IoStoreTocCompressedBlockEntry {
    pub fn new(offset: u64, length: u32) -> Self {
        type ByteBlock = Cursor<[u8; 0xc]>;
        type E = byteorder::NativeEndian;
        let mut byte_builder = Cursor::new([0; 0xc]);
        byte_builder.write_u32::<E>(offset as u32).unwrap();
        byte_builder.seek(SeekFrom::Current(1)); // 0x4
        let cmp_size = &length.to_ne_bytes()[0..3];
        byte_builder.write_all(cmp_size).unwrap(); // cmp_size
        byte_builder.write_all(cmp_size).unwrap(); // decmp_size
        Self { data: byte_builder.into_inner() }
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_all(self.data.as_slice())?;
        Ok(())
    }
    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoStoreTocCompressedBlockEntry>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

// (usually, compression info and signature data would be included here, but we have no reason to
// do that to our emulated UTOC, so no need to define those types)

// IO Directory Index

#[derive(Debug)]
#[repr(C)]
#[allow(dead_code)]
pub struct IoDirectoryIndexEntry {
    pub name: u32, // entry to string index
    pub first_child: u32, // beginning of child list
    pub next_sibling: u32,
    pub first_file: u32
}

impl IoDirectoryIndexEntry {
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u32::<E>(self.name)?;
        writer.write_u32::<E>(self.first_child)?;
        writer.write_u32::<E>(self.next_sibling)?;
        writer.write_u32::<E>(self.first_file)?;
        Ok(())
    }

    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoDirectoryIndexEntry>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u32::<E>(list.len() as u32)?;
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

pub const IO_FILE_INDEX_ENTRY_SERIALIZED_SIZE: usize = 0xc;

#[derive(Debug)]
#[repr(C/*, align(1)*/)]
#[allow(dead_code)]
pub struct IoFileIndexEntry {
    pub name: u32, // entry to string index
    pub next_file: u32,
    pub user_data: u32, // id for FIoChunkId, and FIoOffsetAndLength
    pub os_path: String // THIS WILL NOT GET SERIALIZED
}

impl IoFileIndexEntry {
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u32::<E>(self.name)?;
        writer.write_u32::<E>(self.next_file)?;
        writer.write_u32::<E>(self.user_data)?;
        Ok(())
    }

    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoFileIndexEntry>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u32::<E>(list.len() as u32)?;
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

pub struct IoStringPool;

impl IoStringPool {
    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<String>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_u32::<E>(list.len() as u32)?;
        for i in list {
            FString32NoHash::to_buffer::<W, E>(i, writer)?;
        }
        Ok(())
    }
}

// NON NATIVE - REQUIRES SERIALIZATION
#[allow(dead_code)]
pub struct IoFileResource {
    mount_point: String,
    directory_entries: Vec<IoDirectoryIndexEntry>,
    file_entries: Vec<IoFileIndexEntry>,
    strings: Vec<String>
}

// META (WIP)

#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
pub struct IoStoreTocEntryMeta {
    hash: [u8; 0x20],
    flags: u8
}

impl IoStoreTocEntryMeta {
    pub fn new(buffer: &Vec<u8>) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(buffer);
        let mut data: Cursor<[u8; 0x20]> = Cursor::new([0; 0x20]);
        data.write_all(&hasher.finalize().to_vec());
        let hash = data.into_inner();

        let flags = 0;
        Self { hash, flags }
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_all(self.hash.as_slice())?;
        writer.write_u8(self.flags)?;
        Ok(())
    }
    pub fn list_to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(list: &Vec<IoStoreTocEntryMeta>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in list {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

pub struct ContainerHeader {
    container_id: u64,
    pub packages: Vec<ContainerHeaderPackage>,
}
impl ContainerHeader {
    // Write package header data into ucas
    pub fn new(container_id: u64) -> Self {
        Self { container_id, packages: vec![] }
    }
    pub fn to_buffer<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<Vec<u8>, Box<dyn Error>> {
        // Container Header:
        // - ContainerId + Package Name Count (number of ExportBundleData)
        // - Names array - empty
        // - Name Hashes - one entry containing FNameHash::AlgorithmId
        // - Package Ids - hashes of each export bundle
        // - Store Entries - store entry data for each export bundle (import ids => graph package ids)
        // Culture Package Map - empty
        // Package Redirects - empty
        // Padding to align it to nearest 0x10
        let mut container_header_writer: Cursor<Vec<u8>> = Cursor::new(vec![]);
        container_header_writer.write_u64::<E>(self.container_id)?;
        container_header_writer.write_u32::<E>(self.packages.len() as u32)?;
        // TODO: Implement TArray to_buffer
        container_header_writer.write_u32::<E>(0)?; // TArray<u8> Names
        container_header_writer.write_u32::<E>(8)?; // TArray<u8> NameHashes
        container_header_writer.write_u64::<E>(crate::string::NAME_HASH_ALGORITHM)?;
        container_header_writer.write_u32::<E>(self.packages.len() as u32)?; // TArray<FPackageId> PackageIds
        for i in &self.packages {
            container_header_writer.write_u64::<E>(i.hash)?;
        }
        println!("Written {} package ids into container header", self.packages.len());
        let import_list_base_offset = CONTAINER_HEADER_PACKAGE_SERIALIZED_SIZE * self.packages.len() as u64; // TArray->data, len is written further down
        let mut import_list_already_written_offset = 0;
        let mut store_entry_writer: Cursor<Vec<u8>> = Cursor::new(vec![]);
        for i in &self.packages {
            i.to_buffer_store_entry::<Cursor<Vec<u8>>, E>(&mut store_entry_writer, import_list_base_offset, &mut import_list_already_written_offset)?;
        }
        let store_entry_writer = store_entry_writer.into_inner();
        container_header_writer.write_u32::<E>(store_entry_writer.len() as u32)?;
        container_header_writer.write_all(&store_entry_writer);
        container_header_writer.write_u32::<E>(0)?; // CulturePackageMap
        container_header_writer.write_u32::<E>(0)?; // PackageRedirectss
        let serialized = container_header_writer.into_inner();
        writer.write_all(&serialized); // Write into main buffer, then align to the nearest 0x10
        PartitionSerializer::new(0x10).to_buffer_alignment::<W, E>(writer);
        writer.seek(SeekFrom::Current(-1));
        writer.write(&[0x0])?;
        Ok(serialized)
    }
}

pub const CONTAINER_HEADER_PACKAGE_SERIALIZED_SIZE: u64 = 0x20;
pub struct ContainerHeaderPackage {
    hash: u64,
    export_bundle_size: u64,
    export_count: u32,
    export_bundle_count: u32,
    load_order: u32,
    import_ids: Vec<u64>
}

impl ContainerHeaderPackage {
    // Do a very incomplete serialization of an IO Store packaged asset to obtain it's export count, export bundle count and imported packages
    // Imports are Header.ExportMapOffset - Header.ImportMapOffset / 8
    // Export count is Header.ExportBundlesOffset - Header.ExportMapOffset) / sizeof(FExportMapEntry)
    // Export bundle count is export bundle count - export count
    // imported packages count determined (grab the hash from there and copy that)
    // Later, this code can do a more full serialization
    pub fn from_header_package<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R, hash: u64, size: u64) -> Self { // beginning of IO store package
        reader.seek(SeekFrom::Start(0x2c));
        let export_offset = reader.read_u32::<E>().unwrap();
        let export_bundle_offset = reader.read_u32::<E>().unwrap();
        //println!("0x{:X}, 0x{:X}", export_offset, export_bundle_offset);
        let graph_offset = reader.read_u32::<E>().unwrap();
        let export_count = (export_bundle_offset - export_offset) / 0x20;
        reader.seek(SeekFrom::Start(export_bundle_offset as u64 + 4)); // FExportBundleHeader->EntryCount
        let export_bundle_count_serialized = reader.read_u32::<E>().unwrap();
        let export_bundle_count = export_bundle_count_serialized - export_count;
        reader.seek(SeekFrom::Start(graph_offset as u64)); // FGraphPackage->ImportedPackagesCount
        let imported_package_count = reader.read_u32::<E>().unwrap();
        let mut import_ids: Vec<u64> = Vec::with_capacity(imported_package_count as usize);
        for _ in 0..imported_package_count {
            import_ids.push(FGraphPackage::from_buffer::<R, E>(reader).imported_package_id);
        }
        let load_order = 0; // For now, we'll see if this makes things crash
        Self {
            hash,
            export_bundle_size: size,
            export_count,
            export_bundle_count,
            load_order,
            import_ids
        }
    }

    pub fn to_buffer_store_entry<W: Write + Seek, E: byteorder::ByteOrder>(&self, writer: &mut W, base_offset: u64, curr_offset: &mut u64) -> Result<(), Box<dyn Error>> {
        writer.write_u64::<E>(self.export_bundle_size)?; // 0x0
        writer.write_u32::<E>(self.export_count)?; // 0x8
        writer.write_u32::<E>(self.export_bundle_count)?; // 0xc
        writer.write_u32::<E>(self.load_order)?; // 0x10
        writer.write_u32::<E>(0)?; // 0x14 padding
        let relative_offset = if self.import_ids.len() > 0 { Some((base_offset + *curr_offset - writer.stream_position().unwrap()) as u32) } else { None };
        writer.write_u32::<E>(self.import_ids.len() as u32)?; // 0x18 ImportedPackageCount
        writer.write_u32::<E>(match relative_offset {Some(n) => n, None => 0})?; // 0x1c RelativeOffsetToImports
        if let Some(rel) = relative_offset {
            let return_ptr = writer.stream_position().unwrap();
            writer.seek(SeekFrom::Current(rel as i64 - 8));
            for i in &self.import_ids {
                writer.write_u64::<E>(*i)?;
            }
            writer.seek(SeekFrom::Start(return_ptr));
            *curr_offset += 8 * self.import_ids.len() as u64;
        }
        Ok(())
    }
}