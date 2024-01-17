use bitflags::bitflags;

pub type IoContainerId = u64; // TODO: ContainerID is a UID as a CityHash64 of the container name
                              // represent that with a distinct CityHashID type
pub type GUID = u128;

#[repr(u8)]
#[allow(dead_code)]
// One byte sized enum
// https://doc.rust-lang.org/nomicon/other-reprs.html#repru-repri
enum IoStoreTocVersion {
    Invalid = 0,
    Initial, 
    DirectoryIndex, // added in UE 4.26
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

pub trait IoStoreTocHeader {

}

#[repr(C)]
pub struct IoStoreTocHeaderType1 { // Unreal Engine 4.25 (size: 0x80) (unverified)
    toc_magic: [u8; 0x10],
    toc_header_size: u32,
    toc_entry_count: u32,
    toc_entry_size: u32, // for sanity checking
    toc_pad: [u32; 25]
}

impl IoStoreTocHeader for IoStoreTocHeaderType1 {
    /*
    fn new() -> IoStoreTocHeaderType1 {

    }
    */
}

#[repr(C)]
pub struct IoStoreTocHeaderType2 { // Unreal Engine 4.26 
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
    container_id: IoContainerId,                            
    encryption_key_guid: GUID,
    container_flags: IoContainerFlags,
    reserved: [u32; 15]
}

impl IoStoreTocHeader for IoStoreTocHeaderType2 {
    /*
    fn new() -> IoStoreTocHeaderType2 {

    }
    */
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

impl IoStoreTocHeader for IoStoreTocHeaderType3 {
    /*
    fn new() -> IoStoreTocHeaderType3 {

    }
    */
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

impl IoStoreTocHeader for IoStoreTocHeaderType4 {
    /*
    fn new() -> IoStoreTocHeaderType4 {

    }
    */
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

#[repr(C)]
#[allow(dead_code)]
pub struct IoDirectoryIndexEntry {
    name: u32,
    first_child: u32,
    next_sibling: u32,
    first_file: u32
}

#[repr(C)]
#[allow(dead_code)]
pub struct IoFileIndexEntry {
    name: u32,
    next_file: u32,
    user_data: u32
}

// NON NATIVE - REQUIRES SERIALIZATION
#[allow(dead_code)]
pub struct IoFileResource {
    mount_point: String,
    directory_entries: Vec<IoDirectoryIndexEntry>,
    file_entries: Vec<IoFileIndexEntry>,
    strings: Vec<String>
}
