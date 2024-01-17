// IO Store Package Header types
// Defined in AsyncLoading2.h (Unreal Engine 4.27)

type MappedName = u64; // TODO: make proper struct for this 
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
    pak_package::NameMap,
    partition::GameName
};
use std::{
    error::Error,
    fmt,
    io::{Cursor, Seek, SeekFrom, Write}
};

#[derive(Debug)]
#[repr(u8)]
pub enum FPackageObjectIndexType {
    Export = 0,
    ScriptImport,
    PackageImport,
    Null
}

impl TryFrom<u8> for FPackageObjectIndexType {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FPackageObjectIndexType::Export),
            1 => Ok(FPackageObjectIndexType::ScriptImport),
            2 => Ok(FPackageObjectIndexType::PackageImport),
            3 => Ok(FPackageObjectIndexType::Null),
            _ => Err("Unimplemented FPackageObjectIndex Type")
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct FPackageObjectIndex(u64);

impl FPackageObjectIndex {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
    // TODO: Allow for package conversion for platforms that use 1 byte chars (most seem to use 2 byte chars though)
    pub fn generate_hash(import: &str, obj_type: FPackageObjectIndexType) -> Self {
        let to_hash = String::from(import).to_lowercase();
        // hash chars are sized according to if the platform supports wide characters, which is usually the case
        let to_hash: Vec<u16> = to_hash.encode_utf16().collect();
        // safety: Vec is contiguous, so a Vec<u8> of length `2 * n` will take the same memory as a Vec<u16> of len `n`
        let to_hash = unsafe { std::slice::from_raw_parts(to_hash.as_ptr() as *const u8, to_hash.len() * 2) };
        // verified: the strings are identical (no null terminator) when using FString16
        let mut hash: u64 = cityhasher::hash(to_hash); // cityhash it
        hash &= !(3 << 62); // first 62 bits are our hash
        hash |= (obj_type as u64) << 62; // stick the type in high 2 bits
        Self(hash)
    }
    pub fn get_value(&self) -> u64 {
        self.0 & !(3 << 62)
    }
    pub fn get_type(&self) -> FPackageObjectIndexType {
        FPackageObjectIndexType::try_from((self.0 >> 62 & 3) as u8).unwrap()
    }
}

impl fmt::Display for FPackageObjectIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value 0x{:X}, type {:?}", self.get_value(), self.get_type())
    }
}

impl From<FPackageObjectIndex> for u64 {
    fn from(idx: FPackageObjectIndex) -> u64 {
        idx.0
    }
}

impl AsRef<u64> for FPackageObjectIndex {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

// Io Store Asset Header
pub trait FPackageSummary {
    // 
}

#[repr(C)]
pub struct FPackageSummary1 { // Unreal Engine 4.25 (Scarlet Nexus)
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

impl FPackageSummary for FPackageSummary1 {
    // ...
}

#[repr(C)]
pub struct FPackageSummary2 { // Unreal Engine 4.26-4.27 (P3RE, Hi-Fi RUSH, FF7R)
    name: MappedName,     
    source_name: MappedName,
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

impl FPackageSummary for FPackageSummary2 {
    // ...
}

//use crate::types::FString32NoHash;

pub const UASSET_MAGIC: u32 = 0x9E2A83C1;

#[derive(Debug)]
pub struct ConvertPAKAssetError(&'static str);

impl fmt::Display for ConvertPAKAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error converting PAK Asset: {}", self.0)
    }
}

impl Error for ConvertPAKAssetError {

}

// GPackageFileUE4Version: (from ObjectVersion.h) - corresponds to EUnrealEngineObjectUE4Version
// UE 4.25 - 518
// UE 4.26 - 522
// UE 4.27 - 522
// EUnrealEngineObjectUE5Version:
// UE 5.0 - 1004
// UE 5.1 - 1008
// UE 5.2 - 1009
// UE 5.3 - 1009

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

// Name Map: Vec of FString + u64 Hashes

// Import Map: Vec of u64 Hashes

pub struct ObjectImport<'a> {
    pub class_package: &'a str,
    pub class_name: &'a str,
    pub outer: Option<&'a str>,
    pub object_name: &'a str
}

impl<'a> ObjectImport<'a> {
    pub fn to_buffer<W: Write, E: byteorder::ByteOrder>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        // Write out a single FPackageObjectIndex hash to IO Store
        // Value to hash is ([package_name]/)[path]
        let mut to_hash = String::new();
        if let Some(package_name) = self.outer {
            to_hash.push_str(package_name);
            to_hash.push_str("/");
        }
        to_hash.push_str(&self.object_name);
        to_hash.replace(".", "/"); // regex: find [.:]
        to_hash.replace(":", "/");
        writer.write_u64::<E>(
            FPackageObjectIndex::generate_hash(&to_hash, FPackageObjectIndexType::ScriptImport).into()
        )?;
        Ok(())
    }
}

pub trait ObjectExportWriter {
    fn to_buffer<
        W: Write, 
        E: byteorder::ByteOrder
    >(&self, writer: &mut W, names: &NameMap) -> Result<(), Box<dyn Error>>;

    fn resolve<
        G: GameName
    >(import: &crate::pak_package::FObjectExport, names: &NameMap, file_name: &str, game_name: &G) -> impl ObjectExportWriter;
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

/*
impl<'a> ObjectExportWriter for ObjectExport1<'a> {
    
}
*/

#[derive(Debug)]
#[allow(dead_code)]
pub struct ObjectExport2<'a> { // Unreal Engine 4.25+, 4.26-4.27 
    pub cooked_serial_offset: i64,
    pub cooked_serial_size: i64,
    pub object_name: &'a str,
    pub outer_export: i32, // refers to ith entry in export map
    pub class_name: Option<&'a str>,
    pub super_name: Option<&'a str>,
    pub template_name: Option<&'a str>,
    pub global_import_name: String, // global import index is is usually either null if it's not the root object (outer is null)
    // otherwise it's PackageImport
    pub object_flags: ObjectFlags,
    pub filter_flags: ExportFilterFlags
}

impl<'a> ObjectExportWriter for ObjectExport2<'a> {
    fn to_buffer<
        W: Write, 
        E: byteorder::ByteOrder
    >(&self, writer: &mut W, names: &NameMap) -> Result<(), Box<dyn Error>> {
        writer.write_i64::<E>(self.cooked_serial_offset)?;
        writer.write_i64::<E>(self.cooked_serial_offset)?;
        writer.write_i64::<E>(0)?; // todo for FMappedName
        // TODO
        Ok(())
    }

    fn resolve<
        G: GameName
    >(import: &crate::pak_package::FObjectExport, names: & NameMap, file_name: &str, game_name: &G) -> impl ObjectExportWriter {
        let cooked_serial_offset = import.serial_offset - 4; // PAK package serial offset - magic bytes
        let cooked_serial_size = import.serial_size;
        let object_name = names.get_string_from_index(import.object_name as usize).unwrap();
        // todo: set 
        let outer_export = import.super_index;
        let class_name = names.get_string_from_package_index(import.class_index);
        let super_name = names.get_string_from_package_index(import.super_index);
        let template_name = names.get_string_from_package_index(import.template_index);
        let asset_proj_path = String::from(file_name) + "/" + object_name; // TODO - file path + object name
        let global_import_name = game_name.project_path_to_game_path(&asset_proj_path).unwrap();
        let object_flags = import.object_flags;
        let filter_flags = 0; // EExportFilterFlags::None

        ObjectExport2 {
            cooked_serial_offset,
            cooked_serial_size,
            object_name,
            outer_export,
            class_name,
            super_name,
            template_name,
            global_import_name,
            object_flags,
            filter_flags
        }
    }
}

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

/*
impl<'a> ObjectExportWriter for ObjectExport3<'a> {

}
*/

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
