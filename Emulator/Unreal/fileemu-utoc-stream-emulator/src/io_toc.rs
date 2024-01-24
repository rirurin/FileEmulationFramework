use bitflags::bitflags;
use std::{
    error::Error,
    io::{Seek, Write}
};

pub type IoContainerId = u64; // TODO: ContainerID is a UID as a CityHash64 of the container name
                              // represent that with a distinct CityHashID type
pub type GUID = u128;

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
    LatestPlusOne
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
#[repr(u8)]
#[allow(dead_code)]
enum IoChunkType4 {     
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
    ContainerHeader // added in UE 4.26
}

#[repr(u8)]
#[allow(dead_code)]
enum IoChunkType5 {
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

#[repr(C)]
pub struct IoChunkId {
    id: [u8; 0xc]
}

impl IoChunkId {
} 

// IO OFFSET + LENGTH

#[repr(C)]
pub struct IoOffsetAndLength {
    data: [u8; 0xa]
}

impl IoOffsetAndLength {

}

// (UE 5 ONLY) Perfect Hash

// IO Compression Blocks
#[repr(C)]
pub struct IoStoreTocCompressedBlockEntry {
    data: [u8; 0xc] // 5 bytes offset, 3 bytes for size/uncompressed size, 1 byte for compression
                    // method
}

impl IoStoreTocCompressedBlockEntry {

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

#[derive(Debug)]
#[repr(C/*, align(1)*/)]
#[allow(dead_code)]
pub struct IoFileIndexEntry {
    pub name: u32, // entry to string index
    pub next_file: u32,
    pub user_data: u32 // id for FIoChunkId, and FIoOffsetAndLength
}

// NON NATIVE - REQUIRES SERIALIZATION
#[allow(dead_code)]
pub struct IoFileResource {
    mount_point: String,
    directory_entries: Vec<IoDirectoryIndexEntry>,
    file_entries: Vec<IoFileIndexEntry>,
    strings: Vec<String>
}
