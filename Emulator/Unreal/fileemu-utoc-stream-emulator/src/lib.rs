use std::{
    ffi::CStr,
    mem,
    os::raw::c_char
};

// Package Types (these are contained inside of .PAK and .UCAS)
// PAK Package

// IO Package

// Process to create a new UTOC/UCAS
// On game init, create a dummy PAK and UTOC
// When that utoc is called, populate it:
// - Go through every mod that implements this file emulator and check the folder with the name of
// the target UTOC. Create a list of files (.uasset, .uexp, .ubulk)
// - For each file, convert the PAK asset data into IO Asset data, then append the .uexp data,
// followed by .ubulk. Make sure to align contents according to compression block alignment, and
// ensure that no block is larger than compression block size.
// Then write that into the new UTOC and UCAS file

pub mod io_package; // assets inside of IO Store
pub mod io_toc; // IO Store Table of Contents
pub mod pak_package; // assets inside of PAK or loose file
pub mod toc_factory; // TOC creator
pub mod partition; // Parition builder
pub mod reader; // stream reader
pub mod string; // common Unreal types

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn PrintEmulatedFile(file_path: *const c_char) {
    // make borrowed strings from params - C#'s GC will drop it 
    let utoc_file = CStr::from_ptr(file_path).to_str().unwrap();
    println!("IoStoreTocHeader size: {}", mem::size_of::<io_toc::IoStoreTocHeaderType3>());
    println!("PrintEmulatedFile {}", utoc_file);
}

#[no_mangle]
#[allow(non_snake_case)]
// haiiii Reloaded!!!! :3
pub unsafe extern "C" fn TryGetEmulatedFile(_insert_params_here: usize) {
    // ...
}

#[cfg(test)]
mod tests {
    use byteorder::{ReadBytesExt, WriteBytesExt};
    use crate::{
        io_package::{FPackageObjectIndex, FPackageObjectIndexType, ObjectImport, ObjectExportWriter, ObjectExport2},
        pak_package::{FObjectImport, FObjectExport, NameMap},
        partition::{GameName, GameNameImpl},
        string::{FStringDeserializer, FStringSerializer, FString16, FString32}
    };
    use std::{
        fs::File,
        io::{
            Cursor, Read, Write
        },
    };
    // Testing globals
    // Input: "/Game/StarterContent/Textures/T_Chair_M" from .uasset
    // Length: 40
    // Value: /Game/StarterContent/Textures/T_Chair_M
    // Hash: 0x11D8DC31
    pub const T_CHAIR_MAT_PAK_NAME: [u8; 48] = 
        [0x28, 0x00, 0x00, 0x00, 0x2F, 0x47, 0x61, 0x6D, 0x65, 0x2F, 0x53, 0x74, 0x61, 0x72, 0x74, 0x65,
        0x72, 0x43, 0x6F, 0x6E, 0x74, 0x65, 0x6E, 0x74, 0x2F, 0x54, 0x65, 0x78, 0x74, 0x75, 0x72, 0x65,
        0x73, 0x2F, 0x54, 0x5F, 0x43, 0x68, 0x61, 0x69, 0x72, 0x5F, 0x4D, 0x00, 0x31, 0xDC, 0xD8, 0x11]
    ;

    #[test]
    fn convert_single_name() {
        // Converts a single PAK Package Name into an IO Store name, including rehashing
        let mut cursor = Cursor::new(T_CHAIR_MAT_PAK_NAME); // get FString32 from stream
        let str_in = FString32::from_buffer::<Cursor<[u8; 48]>, byteorder::LittleEndian>(&mut cursor).unwrap();
        if str_in == None {
            panic!("Should've received a non-empty string");
        }
        let str_in = str_in.unwrap();
        assert_eq!(str_in.len(), 39, "Length of string doesn't match expected length"); // this better not be 40...
        assert_eq!(&str_in, "/Game/StarterContent/Textures/T_Chair_M", "String of FString32 doesn't match");
        let mut writer: Vec<u8> = vec![]; // Write to a "file" as an FString16
        FString16::to_buffer::<Vec<u8>, byteorder::LittleEndian>(&str_in, &mut writer);
        assert_eq!(writer[1], 39, "Length was not written to stream properly");
        // TODO: Check text
        assert_eq!(FString16::check_hash(&str_in), 0xA0081646CD9765E7, "Hash of FString16 doesn't match");
    }
    #[test]
    fn convert_name_map_texture() {
        // Converts a name map for a PAK package into an IO Store name map. FString16 names are written first, followed by hashes
        // Name map block from T_Chair_M.uasset
        let mut file = File::open("resources/name_map_test_in.bin").unwrap(); // IO init
        let mut buf = vec![];
        file.read_to_end(&mut buf);
        let mut cursor = Cursor::new(buf); // read 15 names
        //let name_map = NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut cursor, 15);
        let name_map = NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut cursor, 15);
        // FString16 names don't include a null terminator
        assert_eq!(&name_map[0], "/Game/StarterContent/Textures/T_Chair_M", "Value in name map does not match the expected value");
        assert_eq!(&name_map[2], "/Script/Engine", "Value in name map does not match the expected value");
        assert_eq!(&name_map[4], "Default__Texture2D", "Value in name map does not match the expected value");
        assert_eq!(&name_map[12], "StructProperty", "Value in name map does not match the expected value");
        assert_eq!(&name_map[14], "Texture2D", "Value in name map does not match the expected value");
        { // Write out to new file
            let mut file = File::create("resources/name_map_test_out_texture.bin").unwrap();
            let mut buf = Cursor::new(vec![]);
            name_map.to_buffer_two_blocks::<Cursor<Vec<u8>>, FString16, byteorder::NativeEndian>(&mut buf);
            file.write_all(&buf.into_inner());
            // TODO: Write assertions for this (it works at the moment, at least)
        }
    
    }

    #[test]
    fn convert_name_map_particle() {
        // Converts a name map for a PAK package into an IO Store name map. FString16 names are written first, followed by hashes
        // Name map block from T_Chair_M.uasset
        let mut file = File::open("resources/name_map_particle_test.bin").unwrap(); // IO init
        let mut buf = vec![];
        file.read_to_end(&mut buf);
        let mut cursor = Cursor::new(buf); // read 15 names
        //let name_map = NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut cursor, 15);
        let name_map = NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut cursor, 219);
        // FString16 names don't include a null terminator
        assert_eq!(&name_map[0], "/Game/StarterContent/Particles/Materials/M_Burst", "Value in name map does not match the expected value");
        assert_eq!(&name_map[10], "ArrayProperty", "Value in name map does not match the expected value");
        assert_eq!(&name_map[50], "Default__ParticleModuleColorOverLife", "Value in name map does not match the expected value");
        assert_eq!(&name_map[100], "InterpMode", "Value in name map does not match the expected value");
        assert_eq!(&name_map[218], "VelocityScale", "Value in name map does not match the expected value");
        { // Write out to new file
            let mut file = File::create("resources/name_map_test_out_particle.bin").unwrap();
            let mut buf = Cursor::new(vec![]);
            name_map.to_buffer_two_blocks::<Cursor<Vec<u8>>, FString16, byteorder::NativeEndian>(&mut buf);
            file.write_all(&buf.into_inner());
            // TODO: Write assertions for this (it works at the moment, at least)
        }
    }
    
    #[test]
    fn convert_imports() {
        type NE = byteorder::NativeEndian;
        // Converts the import block of a PAK Package to an IO Store import block.
        // Requires building a name map first (get that from T_Chair_M.uasset) 
        let name_map = {
            let mut file = File::open("resources/name_map_test_in.bin").unwrap(); // name map block
            let mut buf = vec![];
            file.read_to_end(&mut buf);
            let mut buf = Cursor::new(buf);
            NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut buf, 15) // read 15 names for T_Chair_M.uasset
        };
        let mut file = File::open("resources/import_map_test_in.bin").unwrap();
        let mut buf = vec![]; // Load in our imports (T_Chair_M has 3 imports)
        file.read_to_end(&mut buf);
        let mut cursor = Cursor::new(buf);
        let f_import_map = FObjectImport::build_import_map::<Cursor<Vec<u8>>, byteorder::LittleEndian>(&mut cursor, 3);
        let import_map = FObjectImport::resolve_imports(&f_import_map, &name_map); // get string values for package ids
        let mut io_import_map: Cursor<Vec<u8>> = Cursor::new(vec![]);
        for i in import_map { // convert to IO Package imports (hash file name)
            let mut to_hash = String::new();
            if let Some(package_name) = &i.outer {
                to_hash.push_str(package_name);
                to_hash.push_str("/");
            }
            to_hash.push_str(&i.object_name);
            to_hash.replace(".", "/"); // regex: find [.:]
            to_hash.replace(":", "/");
            i.to_buffer::<Cursor<Vec<u8>>, byteorder::NativeEndian>(&mut io_import_map);
        }
        // Now check that we made the right hashes
        io_import_map.set_position(0); // go to beginning of string to read these values:
        let import0 = FPackageObjectIndex::new(io_import_map.read_u64::<NE>().unwrap());
        let import1 = FPackageObjectIndex::new(io_import_map.read_u64::<NE>().unwrap());
        let import2 = FPackageObjectIndex::new(io_import_map.read_u64::<NE>().unwrap());
        println!("{}", import0);
        println!("{}", import1);
        println!("{}", import2);
        assert_eq!(import0.get_value(), 0x1b93bca796d1fa6f, "Import Hash does not match");
        assert_eq!(import1.get_value(), 0x11acced3dc7c0922, "Import Hash does not match");
        assert_eq!(import2.get_value(), 0x2bfad34ac8b1f6d0, "Import Hash does not match");
    }
    
    #[test]
    #[allow(unused_variables)]
    // TODO
    fn convert_exports() {
        // Converts the export block of a PAK Package to an IO Store export block
        // Requires building a name map first
        let game_name = GameNameImpl::new("TestingSrc", "Game");
        let file_name = String::from(format!("/{}/Content/StarterContent/Textures/T_Chair_M", game_name.get_project_name()));
        // this + object_name = global_import_name value *as long as super_index is null*
        let name_map = {
            let mut file = File::open("resources/name_map_test_in.bin").unwrap(); // name map block
            let mut buf = vec![];
            file.read_to_end(&mut buf);
            let mut buf = Cursor::new(buf);
            NameMap::new_from_buffer::<Cursor<Vec<u8>>, FString32, byteorder::NativeEndian>(&mut buf, 15) // read 15 names for T_Chair_M.uasset
        };
        let mut file = File::open("resources/export_map_test_in.bin").unwrap();
        let mut buf = vec![]; // Load in our export (Just one export...)
        // A later test will convert the export map for a particle system that has 68 exports
        file.read_to_end(&mut buf);
        let mut cursor_read = Cursor::new(buf);
        let export_data = FObjectExport::from_buffer::<Cursor<Vec<u8>>, byteorder::LittleEndian>(&mut cursor_read).unwrap();
        // FExportMapEntry params
        //let export_data = export_data.resolve(&name_map,&file_name, &game_name);
        let export_data = ObjectExport2::resolve(&export_data, &name_map, &file_name, &game_name);
        // export it
        let mut cursor_write: Cursor<Vec<u8>> = Cursor::new(vec![]);
        export_data.to_buffer::<Cursor<Vec<u8>>, byteorder::NativeEndian>(&mut cursor_write, &name_map);
    }
    /* 

    #[test]
    fn convert_multiple_exports() {
        // Order of exports is preserved between PAK package and IO package, as they're both sorted by serial_offset descending
    }

    #[test]
    fn create_export_bundle_and_graph_data() {
        // Create IO Store package export bundle and graph data. These fields don't have a 1:1 equivalent with PAK packages
    }

    #[test]
    fn convert_pak_to_io_store() {
        // Do a full rehersal of the PAK package to IO Store package conversion. 
        // This includes converting the header, setting appropriate chunk alignments and attaching the respective .uexp file (and .ubulk/.uptnl)
        // Once this is done, just splice together all the IO Store packages to complete the UCAS file
    }

    // Test to write IO Store Chunk IDs
    // Test to write IO Store Chunk Lengths + Offsets
    // Test to write IO Store Compression Blocks
    // Test to write IO Store Directory Info
    */
}
