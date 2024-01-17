// PAK Formatted Assets (Legacy Format)
// Used as asset header for games that create PAK containers and in the editor
// Additionally used in IO Store games for assets not yet supported in IO Store

pub type GUID = u128;
use bitflags::bitflags;
use byteorder::ReadBytesExt;
use crate::{
    io_package::{ObjectImport, ObjectExport2},
    partition::GameName,
    string::{
        FStringDeserializer, FStringSerializer, FStringSerializerHash, 
        FStringSerializerText, FStringSerializerBlockAlign, FMappedName
    }
};
use std::{
    collections::BTreeMap,
    error::Error,
    io::{Cursor, Seek, SeekFrom, Read, Write},
    option::Option,
    ops::Index
};

pub trait PackageFileSummary {

}


bitflags! {
    struct PackageFlags: u32 {
        // Only include flags for versions above 400 (this will be far below 4.25, somewhere in the low 4.1x range most likely)
        // For FPackageFileSummary
        const UE5_ADD_SOFTOBJECTPATH_LIST = 1 << 0; // 1008
        const UE4_ADDED_PACKAGE_SUMMARY_LOCALIZATION_ID = 1 << 1; // 516
        const UE4_SERIALIZE_TEXT_IN_PACKAGES = 1 << 2; // 459
        const UE4_ADDED_SEARCHABLE_NAMES = 1 << 3; // 510
        const UE4_ADDED_PACKAGE_OWNER = 1 << 4; // 518
        const UE4_HAS_OWNER_PERSISTENT_GUID = 1 << 5; // 518 to 519
        const UE4_PACKAGE_SUMMARY_HAS_COMPATIBLE_ENGINE_VERSION = 1 << 6; // 444
        const UE4_PRELOAD_DEPENDENCIES_IN_COOKED_EXPORTS = 1 << 7; // 507
        const UE5_NAMES_REFERENCED_FROM_EXPORT_DATA = 1 << 8; // 1001
        const UE5_PAYLOAD_TOC = 1 << 9; // 1002
        const UE5_DATA_RESOURCES = 1 << 0xA; // 1009
        // For FObjectExport
        const UE4_64BIT_EXPORTMAP_SERIALSIZES = 1 << 0xB; // 511
        const UE5_TRACK_OBJECT_EXPORT_IS_INHERITED = 1 << 0xC; // 1006
        const UE4_COOKED_ASSETS_IN_EDITOR_SUPPORT = 1 << 0xD; // 485 (also thank god)
        const UE5_OPTIONAL_RESOURCES = 1 << 0xE; // 1003
        const UE5_SCRIPT_SERIALIZATION_OFFSET = 1 << 0xF; // 1010
    }
}

/*
pub struct StandardEngine425 {

}
pub struct StandardEngine426 {

}
pub struct StandardEngine427 {

}
pub struct Persona3Reload {

}
*/


pub const PACKAGE_ASSET_MAGIC: u32 = 0xC1832A9E;

// This library pretends that anything before UE 4.25 doesn't exist
// (sorry SMT V)
// PackageFileSummary will require a custom deserializer (using byteorder)

// Packaged asset structure:
// Package File Summary - contains metadata for other sections
// Name Entries
// Object Imports
// Object Exports
// Dependencies
// Preload Dependencies

#[repr(C)]
pub struct CustomVersion {
    key: GUID,
    version: i32
}

#[repr(C)]
pub struct PackageFileSummaryType4 {
    tag: i32, // 0xC1832A9E
    legacy_file_version: i32,
    legacy_ue3_version: i32,
    file_version_ue4: i32,
    file_version_licensee_ue4: i32,
    //custom_version_count: i32,
    custom_versions: Vec<CustomVersion>, // likely change these types later for serialization
    total_header_size: i32,
    folder_name: String,
    package_flags: PackageFlags,
    //name_count: i32,
    names: Vec<String>,
}



impl PackageFileSummary for PackageFileSummaryType4 {

}

#[repr(C)]
pub struct PackageFileSummaryType5 {

}

impl PackageFileSummary for PackageFileSummaryType5 {

}


// Global name map per packaged asset.
pub trait NameMap {
    // Adding onto an already existing name map
    fn add_from_buffer<
        R: Read + Seek,
        T: FStringDeserializer,
        E: byteorder::ByteOrder
    >(&mut self, reader: &mut R, count: usize);
    // Write a contiguous block of names with text info only into a buffer.
    fn to_buffer_text_only<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()>;
    // Write a contiguous block of names, bundling together text and info into a buffer. This is used in PAK packages
    fn to_buffer_single_block<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText + FStringSerializerHash,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()>;
    // Write a block of names, followed by a block of hashes. This is done in IO Store packages
    fn to_buffer_two_blocks<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText + FStringSerializerHash + FStringSerializerBlockAlign,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()>;
    fn get_string_from_index(&self, index: usize) -> Result<&str, String>;
    fn get_string_from_package_index(&self, index: i32) -> Option<&str>;
}
pub struct NameMapImpl(Vec<String>);
impl NameMap for NameMapImpl {
    // Adding onto an already existing name map
    fn add_from_buffer<
        R: Read + Seek,
        T: FStringDeserializer,
        E: byteorder::ByteOrder
    >(&mut self, reader: &mut R, count: usize) {
        for _ in 0..count {
            if let Some(fstr) = T::from_buffer::<R, E>(reader).unwrap() {
                self.0.push(fstr);
            }
        }
    }
    fn to_buffer_text_only<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()> {
        for v in &self.0 {
            T::to_buffer_text::<W, E>(v, writer);
        }
        Ok(())
    }
    fn to_buffer_single_block<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText + FStringSerializerHash,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()> {
        for v in &self.0 {
            T::to_buffer_text::<W, E>(v, writer);
            T::to_buffer_hash::<W, E>(v, writer);
        }
        Ok(())
    }
    fn to_buffer_two_blocks<
        W: Write + Seek,
        T: FStringSerializer + FStringSerializerText + FStringSerializerHash + FStringSerializerBlockAlign,
        E: byteorder::ByteOrder
    >(&self, writer: &mut W) -> std::io::Result<()> {
        for v in &self.0 {
            T::to_buffer_text::<W, E>(v, writer);
        }
        T::to_buffer_alignment::<W, E>(writer);
        for v in &self.0 {
            T::to_buffer_hash::<W, E>(v, writer);
        }
        Ok(())
    }
    fn get_string_from_index(&self, index: usize) -> Result<&str, String> {
        let a = self.0.get(index);
        match self.0.get(index) {
            Some(s) => Ok(s),
            None => Err(
                String::from(format!(
                    "Attempted out of bounds access read. 
                    Name map has {} entries, tried reading index {}", self.0.len(), index)
                )
            )
        }
    }
    fn get_string_from_package_index(&self, index: i32) -> Option<&str> {
        // values above 0 are exports, below zero are imports
        Some(self.get_string_from_index({ 
            match index {
                index if index > 0 => (index - 1) as usize,
                index if index < 0 => -index as usize,
                _ => return None
            }
        }).unwrap())
    }
}
impl Index<usize> for NameMapImpl {
    type Output = String;
    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl NameMapImpl {
    pub fn new() -> Self {
        Self(vec![])
    }
    // Creating a new name map for a new package. This is most likely to be used with asset package strings
    pub fn new_from_buffer<
        R: Read + Seek,
        T: FStringDeserializer,
        E: byteorder::ByteOrder
    >(reader: &mut R, count: usize) -> Self {
        let mut map = NameMapImpl::new();
        map.add_from_buffer::<R, T, E>(reader, count);
        map
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FObjectImport {
    pub class_package: u64,
    pub class_name: u64,
    pub outer_index: i32,
    pub object_name: FMappedName
}

impl FObjectImport {
    pub fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R) -> Result<FObjectImport, Box<dyn Error>> {
        let class_package = reader.read_u64::<E>()?;
        let class_name = reader.read_u64::<E>()?;
        let outer_index = reader.read_i32::<E>()?;
        let object_name = reader.read_u64::<E>()?.into();
        Ok(FObjectImport { class_package, class_name, outer_index, object_name })
    }
    pub fn resolve<'a, N: NameMap>(&'a self, names: &'a N) -> ObjectImport {
        let class_package = names.get_string_from_index(self.class_package as usize).unwrap();
        let class_name = names.get_string_from_index(self.class_name as usize).unwrap();
        let outer = names.get_string_from_package_index(self.outer_index);
        println!("{:?}", outer);
        let object_name = names.get_string_from_index(self.object_name.get_name_index() as usize).unwrap();
        ObjectImport {class_package, class_name, outer, object_name }
    }
    // Convert FObjectImport into named ObjectImport
    pub fn resolve_imports<'a, N: NameMap>(import_map: &'a Vec<FObjectImport>, name_map: &'a N) -> Vec<ObjectImport<'a>> {
        let mut resolves = vec![];
        for i in import_map {
            resolves.push(i.resolve(name_map));
        }
        resolves
    }
    pub fn build_import_map<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R, count: usize) -> Vec<FObjectImport> {
        let mut import_map = vec![];
        for _ in 0..count {
            import_map.push(FObjectImport::from_buffer::<R, E>(reader).unwrap());
        }
        import_map
    }
}

pub struct IntBool(i32);

impl IntBool {
    pub fn new(val: i32) -> Self {
        match val {
            0 | 1 => Self(val),
            _ => panic!("ERROR: Tried to initialize an IntBool with a value other than 0 or 1")
        }
    }
    pub fn value(&self) -> bool {
        match self.0 {
            0 => false,
            1 => true,
            _ => panic!("ERROR: IntBool has value other than 0 or 1")
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct FObjectExport {
    pub class_index: i32,
    pub super_index: i32,
    pub template_index: i32,
    pub outer_index: i32,
    pub object_name: FMappedName,
    pub object_flags: u32,
    pub serial_size: i64, // this is i32 in older versions before 4.25
    pub serial_offset: i64,
    pub bool_forced_export: bool,
    pub bool_not_for_client: bool,
    pub bool_not_for_server: bool,
    pub package_flags: u32,
    pub not_always_loaded_for_editor_game: bool,
    pub is_asset: bool,
    pub first_export_dependency: i32,
    pub serialization_before_serialization_dependencies: i32,
    pub create_before_serialization_dependencies: i32,
    pub serialization_before_create_dependencies: i32,
    pub create_before_create_dependencies: i32
}

impl FObjectExport {
    pub fn from_buffer<R: Read + Seek, E: byteorder::ByteOrder>(reader: &mut R) -> Result<FObjectExport, Box<dyn Error>> {
        let class_index = reader.read_i32::<E>()? - 1; // subtract 1 for exports, but not for imports?
        let super_index = reader.read_i32::<E>()? - 1;
        let template_index = reader.read_i32::<E>()? - 1;
        let outer_index = reader.read_i32::<E>()?;
        let object_name = reader.read_u64::<E>()?.into();
        let object_flags = reader.read_u32::<E>()?;
        let serial_size = reader.read_i64::<E>()?;
        let serial_offset = reader.read_i64::<E>()?;
        let bool_forced_export = IntBool(reader.read_i32::<E>()?).value();
        let bool_not_for_client = IntBool(reader.read_i32::<E>()?).value();
        let bool_not_for_server = IntBool(reader.read_i32::<E>()?).value();
        reader.seek(SeekFrom::Current(0x10)); // Package GUID (not used)
        let package_flags = reader.read_u32::<E>()?;
        let not_always_loaded_for_editor_game = IntBool(reader.read_i32::<E>()?).value();
        let is_asset = IntBool(reader.read_i32::<E>()?).value();
        let first_export_dependency = reader.read_i32::<E>()?;
        let serialization_before_serialization_dependencies = reader.read_i32::<E>()?;
        let create_before_serialization_dependencies = reader.read_i32::<E>()?;
        let serialization_before_create_dependencies = reader.read_i32::<E>()?;
        let create_before_create_dependencies = reader.read_i32::<E>()?;


        Ok(FObjectExport{
            class_index,
            super_index,
            template_index,
            outer_index,
            object_name,
            object_flags,
            serial_size,
            serial_offset,
            bool_forced_export,
            bool_not_for_client,
            bool_not_for_server,
            package_flags,
            not_always_loaded_for_editor_game,
            is_asset,
            first_export_dependency,
            serialization_before_serialization_dependencies,
            create_before_serialization_dependencies,
            serialization_before_create_dependencies,
            create_before_create_dependencies
        })
    }
}
pub struct FExportBundleEntry {

}
impl FExportBundleEntry {

}
pub struct FGraphPackage {

}
impl FGraphPackage {

}

// Object Export:
// ClassIndex: FPackageIndex
// SuperIndex: FPackageIndex
// TemplateIndex: FPackageIndex
// OuterIndex: FPackageIndex
// ObjectName: FName
// ObjectFlags: flags
// SerialSize: .uexp size - magic bytes at end
// SerialOffset - size of .uasset (it's a separate file but yeah)
// and then a bunch of flags...

// IO Object Export:
// ObjectName: FMappedName
// CookedSerialOffset - .uasset size - magic bytes at start
// CookedSerialSize - .uexp size - magic bytes at end
// ObjectName - FMappedName
// OuterName - FPackageObjectIndex
// ClassName - FPackageObjectIndex
// SuperIndex - FPackageObjectIndex
// TemplateIndex - FPackageObjectIndex
// GlobalImportIndex - FPackageObjectIndex
// ObjectFlags - flags
// FilterFlags - ??