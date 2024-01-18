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
        io_package::{IoStoreObjectIndex, ObjectImport, ObjectExport2},
        pak_package::{FObjectImport, FObjectExport, NameMap, NameMapImpl},
        partition::{GameName, GameNameImpl},
        string::{FStringDeserializer, FStringSerializer, FStringSerializerText, FString16, FString32}
    };
    use std::{
        fs::File,
        fmt,
        io::{
            Cursor, Read, Write
        },
    };
    type NE = byteorder::NativeEndian;
    type CV = Cursor<Vec<u8>>;

    // Helper functions for tests
    fn get_test_file<N: AsRef<str> + fmt::Display>(file: N) -> Vec<u8> {
        let mut file = File::open(format!("test_resources/{}.bin", file)).unwrap(); // IO init
        let mut buf = vec![];
        file.read_to_end(&mut buf);
        buf
    }
    fn write_to_file<N: AsRef<str> + fmt::Display>(file: N, stream: &Vec<u8>) {
        let mut writer = File::create(format!("test_results/{}.bin", file)).unwrap();
        writer.write_all(stream).unwrap();
    }
    fn write_and_assert(file: &str, stream: Vec<u8>) {
        write_to_file(file, &stream); // write to [filename].bin in test_results
        // and check against [filename]_out.bin in test_resources
        assert_eq!(get_test_file(String::from(file) + "_out"), stream, "Exported byte streams don't match");
    }

    #[test]
    fn convert_name_map_texture() {
        // Converts a name map for a PAK package into an IO Store name map. FString16 names are written first, followed by hashes
        let mut reader = Cursor::new(get_test_file("name_map_texture_in")); // Name map block from T_Chair_M.uasset
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 15);
        let mut writer = Cursor::new(vec![]);
        name_map.to_buffer_two_blocks::<CV, FString16, NE>(&mut writer); // two blocks for IO Store stream
        write_and_assert("name_map_texture", writer.into_inner());
    }

    #[test]
    fn convert_name_map_particle() {
        let mut reader = Cursor::new(get_test_file("name_map_particle_in")); // Name map from P_Explosion.uasset
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 219);
        let mut writer = Cursor::new(vec![]);
        name_map.to_buffer_two_blocks::<CV, FString16, NE>(&mut writer);
        write_and_assert("name_map_particle", writer.into_inner());
    }

    #[test]
    fn convert_imports_exports_texture() {
        // Convert the import and export map from T_Chair_M.uasset
        let game_name = GameNameImpl::new("TestingSrc", "Game");
        let file_name = String::from(format!("/{}/Content/StarterContent/Textures/T_Chair_M", game_name.get_project_name()));

        let mut name_map_reader = Cursor::new(get_test_file("name_map_texture_in"));
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut name_map_reader, 15); // import name map first
        let mut reader = Cursor::new(get_test_file("import_export_map_texture_in")); // we also need to read imports
        let import_map = FObjectImport::build_map::<CV, NE>(&mut reader, 3); // 3 imports
        let import_map = ObjectImport::from_pak_asset(&import_map, &name_map); // resolve our imports (makes getting strings for export easier)
        /*
        let export_map = FObjectExport::build_map::<CV, NE>(&mut reader, 1); // a single export
        let export_map = ObjectExport2::from_pak_asset(&export_map, &name_map, &import_map, &file_name, &game_name);
        let mut writer: CV = Cursor::new(vec![]);
        ObjectExport2::map_to_buffer::<CV, NE>(&export_map, &mut writer);
        write_to_file("import_export_map_texture_out", &writer.into_inner());
        */
    }
    
    #[test]
    fn convert_imports_exports_particle() {
        // Convert import map from P_Explosion.uasset
        // This contains 57 imports of varying types (ScriptImport, PackageImport and Empty)
        let game_name = GameNameImpl::new("TestingSrc", "Game");
        let file_name = String::from(format!("/{}/Content/StarterContent/Particles/P_Explosion", game_name.get_project_name()));

        let mut reader = Cursor::new(get_test_file("name_map_particle_in")); // P_Explosion name map
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 219);

        reader = Cursor::new(get_test_file("import_export_map_particle_in")); // read imports
        let import_map = FObjectImport::build_map::<CV, NE>(&mut reader, 57);
        let import_map = ObjectImport::from_pak_asset(&import_map, &name_map);
        let export_map = FObjectExport::build_map::<CV, NE>(&mut reader, 69);
        let export_map = ObjectExport2::from_pak_asset(&export_map, &name_map, &import_map, &file_name, &game_name);
        let mut writer: CV = Cursor::new(vec![]);
        ObjectImport::map_to_buffer::<CV, NE>(&import_map, &mut writer);
        ObjectExport2::map_to_buffer::<CV, NE>(&export_map, &mut writer);
        write_to_file("import_export_map_particle", &writer.into_inner());
        //write_and_assert("import_map_particle", writer.into_inner());
    }
    // TODO: Figure out what's going on with export bundles and graph data 
    // Also the last block of bytes at the end of IO Store packages (partition layout related?)
    /*
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
