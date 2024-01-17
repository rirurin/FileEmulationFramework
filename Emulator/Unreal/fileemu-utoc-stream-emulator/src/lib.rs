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
        pak_package::{FObjectImport, FObjectExport, NameMap, NameMapImpl},
        partition::{GameName, GameNameImpl},
        string::{FStringDeserializer, FStringSerializer, FStringSerializerText, FString16, FString32}
    };
    use std::{
        fs::File,
        io::{
            Cursor, Read, Write
        },
    };
    type NE = byteorder::NativeEndian;
    type CV = Cursor<Vec<u8>>;

    fn get_test_file(file: &str) -> Vec<u8> {
        let mut file = File::open(format!("resources/{}.bin", file)).unwrap(); // IO init
        let mut buf = vec![];
        file.read_to_end(&mut buf);
        buf
    }
    fn build_name_map(file: &str) -> NameMapImpl {
        let mut buf = Cursor::new(get_test_file(file));
        NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut buf, 15) // read 15 names for T_Chair_M.uasset
    }
    fn write_to_file(file: &str, stream: &Vec<u8>) {
        let mut writer = File::create(format!("resources/{}.bin", file)).unwrap();
        writer.write_all(stream).unwrap();
    }

    #[test]
    fn convert_single_name() {
        // Converts a single PAK Package Name into an IO Store name
        // Includes text and hashes
        let mut buf = Cursor::new(get_test_file("single_name_test_in")); // get FString32 from stream
        let str_in = FString32::from_buffer::<CV, NE>(&mut buf).unwrap();
        assert_ne!(str_in, None, "String is empty (0 characters in stream)");
        let str_in = str_in.unwrap();
        assert_eq!(str_in.len(), 39, "Length of string doesn't match expected length"); // this better not be 40...
        assert_eq!(&str_in, "/Game/StarterContent/Textures/T_Chair_M", "String of FString32 doesn't match");
        let mut writer: CV = Cursor::new(vec![]); // Write to a "file" as an FString16
        FString16::to_buffer_text::<CV, NE>(&str_in, &mut writer); // export only the text portion, hashes are stored on a different block in IO store...
        assert_eq!(get_test_file("single_name_test_cmp"), writer.into_inner(), "Exported byte streams don't match");
    }
    #[test]
    fn convert_name_map_texture() {
        // Converts a name map for a PAK package into an IO Store name map. FString16 names are written first, followed by hashes
        let mut reader = Cursor::new(get_test_file("name_map_test_in")); // Name map block from T_Chair_M.uasset
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 15);
        let mut writer = Cursor::new(vec![]);
        name_map.to_buffer_two_blocks::<CV, FString16, NE>(&mut writer);
        assert_eq!(get_test_file("name_map_test_cmp"), writer.into_inner(), "Exported byte streams don't match");
    }

    #[test]
    fn convert_name_map_particle() {
        // Name map from P_Explosion.uasset
        let mut reader = Cursor::new(get_test_file("name_map_particle_in"));
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 219);
        let mut writer = Cursor::new(vec![]);
        name_map.to_buffer_two_blocks::<CV, FString16, NE>(&mut writer);
        assert_eq!(get_test_file("name_map_particle_cmp"), writer.into_inner(), "Exported byte streams don't match");
    }
    
    #[test]
    fn convert_imports() {
        // Converts the import block of a PAK Package to an IO Store import block.
        // Requires building a name map first (get that from T_Chair_M.uasset) 
        let name_map = build_name_map("name_map_test_in");
        let mut reader = Cursor::new(get_test_file("import_map_test_in")); // Load in our imports (T_Chair_M has 3 imports)
        let import_map = FObjectImport::build_import_map::<Cursor<Vec<u8>>, byteorder::LittleEndian>(&mut reader, 3);
        let import_map = FObjectImport::resolve_imports(&import_map, &name_map); // get string values for package ids
        let mut writer: CV = Cursor::new(vec![]);
        for i in import_map { // convert to IO Package imports (hash file name)
            i.to_buffer::<CV, NE>(&mut writer);
        }
        assert_eq!(get_test_file("import_map_test_cmp"), writer.into_inner(), "Exported byte streams don't match");
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
        let name_map = build_name_map("name_map_test_in");
        let mut reader = Cursor::new(get_test_file("export_map_test_in")); // Load in our export (Just one export...)
        let export_data = FObjectExport::from_buffer::<CV, NE>(&mut reader).unwrap(); // Get FExportMapEntry params
        let export_data = ObjectExport2::resolve(&export_data, &name_map, &file_name, &game_name);
        let mut writer: CV = Cursor::new(vec![]); // export it
        export_data.to_buffer::<CV, NameMapImpl, NE>(&mut writer, &name_map);
        let buf_out = writer.into_inner();
        write_to_file("export_map_test_out", &buf_out);
        assert_eq!(get_test_file("export_map_test_cmp"), buf_out, "Exported byte streams don't match");
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
