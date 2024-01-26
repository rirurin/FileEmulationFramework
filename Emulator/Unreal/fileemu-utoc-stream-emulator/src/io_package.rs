// IO Store Package Header types
// Defined in AsyncLoading2.h (Unreal Engine 4.27)

//type MappedName = u64; // TODO: make proper struct for this (DONE)
type PackageObjectIndex = u64; // TODO: make proper struct for this
type ObjectFlags = u32; // this probably doesn't need to be defined...
type ExportFilterFlags = u8; // and this one too...

// Structure of IO Store Asset:
// Header: FPackageSummary (requires converting PAK Package to IO Package)
// Data: contents of .uexp - 4 magic bytes at end
// Texture Bulk: all of .ubulk
// Last 60 bytes or so contains some currently unknown data

use byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use crate::{
    pak_package::{FObjectImport, FObjectExport, NameMap},
    partition::GameName,
    string::FMappedName
};
use std::{
    error::Error,
    fmt,
    io::{Cursor, Read, Seek, SeekFrom, Write}
};
// IoStoreObjectIndex is a 64 bit value consisting of a hash of a target string for the lower 62 bits and an object type for the highest 2
// expect for Empty which represents a null value and Export which contains an index to another item on the export tree
// This struct is used to fully represent an import on an IO Store package, and is the basic structure for several named fields in export
#[derive(Debug, Clone, PartialEq)]
pub enum IoStoreObjectIndex {
    Export(u64),            // type 0 (index, Export -> Export)
    ScriptImport(String),   // type 1 (string hash, represents Import mounted at /Script/...)
    PackageImport(String),  // type 2 (string hash, represents Import mounted at /Game/...)
    Empty                   // type 3 (-1)
}

impl IoStoreObjectIndex {
    pub fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(&self, reader: &mut R) -> IoStoreObjectIndex {
        let raw_value = reader.read_u64::<E>().unwrap();
        let obj_type = raw_value & (3 << 62);
        match obj_type {
            0 => IoStoreObjectIndex::Export(0), // can't derive string name from hash, will likely need to separate this off to another type for container header building
            1 => IoStoreObjectIndex::ScriptImport(String::new()),
            2 => IoStoreObjectIndex::PackageImport(String::new()),
            3 => IoStoreObjectIndex::Empty,
            _ => panic!("Invalid obj type {}", obj_type),
        }
    }
    // TOOO: upgrade trait bounds to Write + Seek
    pub fn to_buffer<W: Write, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Export(i) => writer.write_u64::<E>(*i as u64)?,
            Self::ScriptImport(v) => writer.write_u64::<E>(IoStoreObjectIndex::generate_hash(v, 1))?,
            Self::PackageImport(v) => writer.write_u64::<E>(IoStoreObjectIndex::generate_hash(v, 2))?,
            Self::Empty => writer.write_u64::<E>(u64::MAX)?,
        }
        Ok(())
    }

    fn generate_hash(import: &str, obj_type: u64) -> u64 {
        println!("make hash for {}", import);
        let to_hash = String::from(import).to_lowercase();
        // hash chars are sized according to if the platform supports wide characters, which is usually the case
        let to_hash: Vec<u16> = to_hash.encode_utf16().collect();
        // safety: Vec is contiguous, so a Vec<u8> of length `2 * n` will take the same memory as a Vec<u16> of len `n`
        let to_hash = unsafe { std::slice::from_raw_parts(to_hash.as_ptr() as *const u8, to_hash.len() * 2) };
        // verified: the strings are identical (no null terminator) when using FString16
        let mut hash: u64 = cityhasher::hash(to_hash); // cityhash it
        hash &= !(3 << 62); // first 62 bits are our hash
        hash |= obj_type << 62; // stick the type in high 2 bits
        hash
    }
}

pub struct ObjectImport;
impl ObjectImport {
    // Convert FObjectImport into named ObjectImport
    pub fn from_pak_asset<N: NameMap>(import_map: &Vec<FObjectImport>, name_map: &N) -> Vec<IoStoreObjectIndex> {
        let mut resolves = vec![];
        for (i, v) in import_map.into_iter().enumerate() {
            match v.resolve(name_map, import_map) {
                Ok(obj) => resolves.push(obj),
                Err(e) => panic!("Error converting PAK formatted import to IO Store import on ID {} \nValue {:?}\nReason: {}", i, v, e.to_string())
            }
        }
        resolves
    }

    pub fn map_to_buffer<W: Write, E: byteorder::ByteOrder>(map: &Vec<IoStoreObjectIndex>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in map {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}

// Io Store Asset Header
#[repr(C)]
pub struct FPackageSummary1 { // Unreal Engine 4.25
    package_flags: u32,
    name_map_offset: i32,
    import_map_offset: i32,
    export_map_offset: i32,
    export_bundles_offset: i32,
    graph_data_offset: i32,
    graph_data_size: i32,
    bulk_data_start_offset: i32,
    global_import_index: i32,
    padding: i32
}


#[repr(C)]
pub struct FPackageSummary2 { // Unreal Engine 4.25+, 4.26-4.27 (Scarlet Nexus, P3RE, Hi-Fi RUSH, FF7R)
    name: FMappedName,     
    source_name: FMappedName,
    package_flags: u32,
    cooked_header_size: u32,
    name_map_names_offset: i32,
    name_map_names_size: i32,
    name_map_hashes_offset: i32,
    name_map_hashes_size: i32,
    import_map_offset: i32,
    export_map_offset: i32,
    export_bundles_offset: i32,
    graph_data_offset: i32,
    graph_data_size: i32
}

impl FPackageSummary2 {
    pub fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R) -> Self {
        let name = reader.read_u64::<E>().unwrap().into();
        let source_name = reader.read_u64::<E>().unwrap().into();
        let package_flags = reader.read_u32::<E>().unwrap();
        let cooked_header_size = reader.read_u32::<E>().unwrap();
        let name_map_names_offset = reader.read_i32::<E>().unwrap();
        let name_map_names_size = reader.read_i32::<E>().unwrap();
        let name_map_hashes_offset = reader.read_i32::<E>().unwrap();
        let name_map_hashes_size = reader.read_i32::<E>().unwrap();
        let import_map_offset = reader.read_i32::<E>().unwrap();
        let export_map_offset = reader.read_i32::<E>().unwrap();
        let export_bundles_offset = reader.read_i32::<E>().unwrap();
        let graph_data_offset = reader.read_i32::<E>().unwrap();
        let graph_data_size = reader.read_i32::<E>().unwrap();
        Self {
            name,
            source_name,
            package_flags,
            cooked_header_size,
            name_map_names_offset,
            name_map_names_size,
            name_map_hashes_offset,
            name_map_hashes_size,
            import_map_offset,
            export_map_offset,
            export_bundles_offset,
            graph_data_offset,
            graph_data_size,
        }
    }
}

pub const UASSET_MAGIC: u32 = 0x9E2A83C1;

#[derive(Debug)]
pub struct ConvertPAKAssetError(&'static str);

impl fmt::Display for ConvertPAKAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error converting PAK Asset: {}", self.0)
    }
}

impl Error for ConvertPAKAssetError {}

// GPackageFileUE4Version: (from ObjectVersion.h) - corresponds to EUnrealEngineObjectUE4Version
// UE 4.25 - 518
// UE 4.26 - 522
// UE 4.27 - 522
// EUnrealEngineObjectUE5Version:
// UE 5.0 - 1004
// UE 5.1 - 1008
// UE 5.2 - 1009
// UE 5.3 - 1009
/* 
impl FPackageSummary2 {
    #[allow(unused_must_use)]
    #[allow(unused_variables)]
    pub fn from_pak_asset<T: AsRef<[u8]>>(reader: &mut Cursor<T>) -> Result<(), Box<dyn Error>> {
        // Can't validate the size of uasset summary ahead of time as it's not constant
        let magic = reader.read_u32::<NativeEndian>()?;
        if magic != UASSET_MAGIC {
            return Err(Box::new(ConvertPAKAssetError("Incorrect file magic")))
        }
        let _ = reader.seek(SeekFrom::Current(std::mem::size_of::<u32>() as i64 * 4))?; // LegacyFileVersion,
                                                                         // LegacyUE3Version,
                                                                         // FileVersionUE4,
                                                                         // FileVersionLicenseeUE
        let custom_ver_count = reader.read_i32::<NativeEndian>()?;
        let mut skip: i64 = 4; // skip TotalHeaderSize, don't need it;
        if custom_ver_count > 0 {
            skip += std::mem::size_of::<u32>() as i64 * 5 * custom_ver_count as i64; 
            // FCustomVersion: GUID (16 bytes) + Version (4 bytes)
        }
        reader.seek(SeekFrom::Current(skip));
        //let folder_name = FString32NoHash::from_reader(reader)?;
        //println!("Folder name: {}", folder_name);
        // Important fields coming up....
        let package_flags = reader.read_u32::<NativeEndian>()?;
        let name_count = reader.read_u32::<NativeEndian>()?;
        let name_offset = reader.read_u32::<NativeEndian>()?;
        // IF UE 5.1 OR LATER: Soft Object Path Count
        // If there's no PKG_FilterEditorOnly flag, LocalizationId would go here
        let gatherable_text_count = reader.read_u32::<NativeEndian>()?;
        let gatherable_text_offset = reader.read_u32::<NativeEndian>()?;
        let import_count = reader.read_u32::<NativeEndian>()?;
        let import_offset = reader.read_u32::<NativeEndian>()?;
        let export_count = reader.read_u32::<NativeEndian>()?;
        let export_offset = reader.read_u32::<NativeEndian>()?;
        let depends_offset = reader.read_u32::<NativeEndian>()?;
        let soft_pkg_ref_count = reader.read_u32::<NativeEndian>()?;
        let soft_pkg_ref_offset = reader.read_u32::<NativeEndian>()?;
        let thumbnail_offset = reader.read_u32::<NativeEndian>()?;
        let guid = reader.read_u128::<NativeEndian>()?;
        Ok(())

        // IO Package:
        // PackagedFlags - copy from package_flags
        // CookedHeaderSize - size of .uasset
        // NameMapNamesOffset - 0x40 (IO Package header is constant size)
        // NameMapNamesSize - will get once names are resolved
        // NameMapHashesOffset - will get once names are resolved
        // ImportMapOffset - will get once imports are resolved ()
        // ExportMapOffset - will get once exports are resolved
        // ExportBundlesOffset - will get once exports (depends?) are resolved
        // GraphDataOffset - uhh??
        // GraphDataSize - uhh??
        //
        // PAK FObjectImport:
        //      ClassPackage points to the string matching the target Package name
        //      ClassName points to the string matching the class's name
        //      OuterIndex - 
        //      ObjectName points to the string matching the object's name
        //
        // IO Table of Contents:
        // Toc Header: 0x90, normal business (check how ContainerId is derived)
        // IoChunkIds - cityhash calc of file locations
        // IoOffsetAndLength - location on partition
        // IoStoreTocCompressedBlockEntry - offset and compress/decompress size of each chunk
        // Mount Point (string)
        // DirectoryEntries
        // FileEntries
        // Strings
        // Toc Meta
        //
        // Check UE4Parse for how it handles ObjectImport and ObjectExport
        // Parse PAK Package imports and exports
        // Then try converting to IO Store
        // Additionally, check UE4's source code (though it's usually a bit harder to read)
    }
}
*/
/* 
#[repr(C)]
pub struct ZenPackageSummaryType1 { // Unreal Engine 5.0-5.2 (Garten of Banban LMAO)
    bool_has_version_info: u32,
    header_size: u32,
    name: MappedName,
    package_flags: u32,
    cooked_header_size: u32,
    imported_public_export_hases_offset: i32,
    import_map_offset: i32,
    export_map_offset: i32,
    export_bundle_entries_offset: i32,
    graph_data_offset: i32
}

#[repr(C)]
pub struct ZenPackageSummaryType2 { // Unreal Engine 5.3
     bool_has_version_info: u32,
    header_size: u32,
    name: MappedName,
    package_flags: u32,
    cooked_header_size: u32,
    imported_public_export_hases_offset: i32,
    import_map_offset: i32,
    export_map_offset: i32,
    export_bundle_entries_offset: i32,
    dependency_bundle_headers_offset: i32,
    dependency_bundle_entries_offset: i32,
    imported_package_names_offset: i32
}
*/
// Name Map: Vec of FString + u64 Hashes

// Import Map: Vec of u64 Hashes (derived from the import file name)
// TODO: Rename this to IoStoreObjectIndex
/* 
#[derive(Debug, PartialEq)]
pub enum ObjectImport {
    ScriptImport(String),
    PackageImport(String),
    Empty
}
/* 
#[derive(Debug)]
pub struct ObjectImport {
    pub import_file: Option<String>
}
*/

impl ObjectImport {
    pub fn to_buffer<W: Write, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        // Write out a single IoStoreObjectIndex hash to IO Store
        // Value to hash is ([package_name]/)[path]
        match self {
            Self::ScriptImport(path)  => ObjectImport::write_hash::<W, E>(path, writer, IoStoreObjectIndexType::ScriptImport),
            Self::PackageImport(path) => ObjectImport::write_hash::<W, E>(path, writer, IoStoreObjectIndexType::PackageImport),
            Self::Empty => {
                writer.write_u64::<E>(u64::MAX)?; // write_hash(path, writer, IoStoreObjectIndexType::Null)
                Ok(())
            },
        };
        Ok(())
    }

    fn write_hash<W: Write, E: byteorder::ByteOrder>(path: &str, writer: &mut W, obj_type: IoStoreObjectIndexType) -> Result<(), Box<dyn Error>> {
        let mut to_hash = String::from(path);
        to_hash = to_hash.replace(".", "/"); // regex: find [.:]
        to_hash = to_hash.replace(":", "/");
        writer.write_u64::<E>(
            IoStoreObjectIndex::generate_hash(&to_hash, obj_type).into()
        )?;
        Ok(())
    }

    // Convert FObjectImport into named ObjectImport
    pub fn from_pak_asset<N: NameMap>(import_map: &Vec<FObjectImport>, name_map: &N) -> Vec<ObjectImport> {
        let mut resolves = vec![];
        for (i, v) in import_map.into_iter().enumerate() {
            println!("{}, {:?}", i, v);
            match v.resolve(name_map, import_map) {
                Ok(obj) => resolves.push(obj),
                Err(e) => panic!("Error converting PAK formatted import to IO Store import on ID {} \nValue {:?}\nReason: {}", i, v, e.to_string())
            }
        }
        resolves
    }

    pub fn map_to_buffer<W: Write, E: byteorder::ByteOrder>(map: &Vec<Self>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in map {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }
}*/
/*
pub trait ObjectExportWriter {
    fn to_buffer<
        W: Write,
        N: NameMap,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W, names: &N) -> Result<(), Box<dyn Error>>;

    fn resolve<
        G: GameName,
        N: NameMap,
    >(import: &crate::pak_package::FObjectExport, names: &N, file_name: &str, game_name: &G) -> impl ObjectExportWriter;
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ObjectExport1<'a> { // Unreal Engine 4.25
    pub serial_size: i64,
    pub object_name: &'a str,
    pub outer_export: i32, // refers to ith entry in export map
    pub class_name: Option<&'a str>,
    pub super_name: Option<&'a str>,
    pub template_name: Option<&'a str>,
    pub global_import_name: String,
    pub object_flags: ObjectFlags,
    pub filter_flags: ExportFilterFlags
}


impl<'a> ObjectExportWriter for ObjectExport1<'a> {
    
}

#[derive(Debug)]
pub struct MappedName<'a> {
    pub name: &'a str,
    pub value: FMappedName
}
*/

#[derive(Debug)]
pub struct ObjectExport2 { // Unreal Engine 4.25+, 4.26-4.27 
    pub cooked_serial_offset: i64,
    pub cooked_serial_size: i64,
    pub object_name: FMappedName,
    pub outer_index: IoStoreObjectIndex, // TODO: use refs preferably
    pub class_name: IoStoreObjectIndex,
    pub super_name: IoStoreObjectIndex,
    pub template_name: IoStoreObjectIndex,
    pub global_import_name: IoStoreObjectIndex,
    pub object_flags: ObjectFlags,
    pub filter_flags: ExportFilterFlags
}

impl ObjectExport2 {
    pub fn from_pak_asset<
        N: NameMap,
        G: GameName
    >(map: &Vec<FObjectExport>, names: &N, imports: &Vec<IoStoreObjectIndex>, file_name: &str, game_name: &G) -> Vec<ObjectExport2> {
        // Convert FObjectImport into named ObjectImport
        let mut resolves = vec![];
        for (i, v) in map.into_iter().enumerate() {
            println!("{}, {:?}", i, v);
            resolves.push(v.resolve(names, imports, map, file_name, game_name));
        }
        resolves
    }

    pub fn map_to_buffer<W: Write, E: byteorder::ByteOrder>(map: &Vec<Self>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        for i in map {
            i.to_buffer::<W, E>(writer)?;
        }
        Ok(())
    }

    pub fn to_buffer<W: Write, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        writer.write_i64::<E>(self.cooked_serial_offset);
        writer.write_i64::<E>(self.cooked_serial_size);
        writer.write_u64::<E>(self.object_name.into())?; // object_name
        self.outer_index.to_buffer::<W, E>(writer)?;
        self.class_name.to_buffer::<W, E>(writer)?;
        self.super_name.to_buffer::<W, E>(writer)?;
        self.template_name.to_buffer::<W, E>(writer)?;
        self.global_import_name.to_buffer::<W, E>(writer)?;
        writer.write_u32::<E>(self.object_flags)?;
        writer.write_u32::<E>(0)?; // filter flags
        Ok(())
    }
}
/*
#[derive(Debug)]
#[allow(dead_code)]
pub struct ObjectExport3<'a> { // Unreal Engine 5.0+
    pub cooked_serial_offset: i64,
    pub cooked_serial_size: i64,
    pub object_name: &'a str,
    pub outer_export: i32, // refers to ith entry in export map
    pub class_name: Option<&'a str>,
    pub super_name: Option<&'a str>,
    pub template_name: Option<&'a str>,
    pub public_export_hash: u64,
    pub object_flags: ObjectFlags,
    pub filter_flags: ExportFilterFlags
}


impl<'a> ObjectExportWriter for ObjectExport3<'a> {

}


pub trait ExportBundleHeader {

}

#[repr(C)]
pub struct ExportBundleHeader4 { // Unreal Engine 4.25-4.27
    first_entry_index: u32,
    entry_count: u32,
}

impl ExportBundleHeader for ExportBundleHeader4 {

}

#[repr(C)]
pub struct ExportBundleHeader5 { // Unreal Engine 5.0-5.2
    serial_offset: u64,
    first_entry_index: u32,
    entry_count: u32,
}

impl ExportBundleHeader for ExportBundleHeader5 {

}
*/

pub struct FGraphExternalArc {
    from_export_bundle_index: u32,
    to_export_bundle_index: u32
}

impl FGraphExternalArc {
    fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R) -> Self {
        let from_export_bundle_index = reader.read_u32::<E>().unwrap();
        let to_export_bundle_index = reader.read_u32::<E>().unwrap();
        Self { from_export_bundle_index, to_export_bundle_index }
    }
}

pub struct FGraphPackage {
    pub imported_package_id: u64, // hashed
    external_arcs: Vec<FGraphExternalArc>
}

impl FGraphPackage {
    pub fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R) -> Self {
        let imported_package_id = reader.read_u64::<E>().unwrap();
        let external_arc_count = reader.read_u32::<E>().unwrap();
        let mut external_arcs = Vec::with_capacity(external_arc_count as usize);
        for _ in 0..external_arc_count {
            external_arcs.push(FGraphExternalArc::from_buffer::<R, E>(reader));
        }
        Self {
            imported_package_id,
            external_arcs
        }
    }
}