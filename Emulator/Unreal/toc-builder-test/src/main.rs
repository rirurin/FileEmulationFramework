
use std::{
    env,
    time::Instant
};
use fileemu_utoc_stream_emulator;
#[allow(unused_imports, unused_braces)]
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem
    }
};

#[allow(unused_variables)]
fn main() {
    // TOC building test program without having to start Reloaded
    let r3_mod_path = env::var("RELOADEDIIMODS");
    if let Err(_) = r3_mod_path {
        println!("Environment variables is missing an entry for \"RELOADEDIIMODS\"");
        return;
    }
    let r3_mod_path = r3_mod_path.unwrap();
    println!("Using Reloaded mods directory at {}", r3_mod_path);
    // get our first test mods, convert to byte*
    let test_mod_1_id = "uetest.loosefiletest1";
    let test_mod_1 = r3_mod_path.clone() + "/" + test_mod_1_id;
    //let test_mod_2 = r3_mod_path.clone() + "/uetest.loosefiletest2";
    let unreal_essentials_toc = r3_mod_path.clone() + "/UnrealEssentials/Unreal/UnrealEssentials_P.utoc";
    let unreal_essentials_partition = r3_mod_path.clone() + "/UnrealEssentials/Unreal/UnrealEssentials_P.ucas";

    fileemu_utoc_stream_emulator::asset_collector::add_from_folders(test_mod_1_id, &test_mod_1);
    unsafe { fileemu_utoc_stream_emulator::asset_collector::print_asset_collector_results(); }

    // open TOC file handle
    /* 
    let toc_filename_win32 = unreal_essentials_toc.clone() + "\0";
    let start_making_toc = Instant::now();
    let mut add_from_first_folder = 0;
    let mut add_from_second_folder = 0;
    let mut build_toc = 0;
    unsafe {
        // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/fn.CreateFileA.html
        match FileSystem::CreateFileA(
            PCSTR::from_raw(toc_filename_win32.as_ptr()),
            FileSystem::FILE_GENERIC_WRITE.0,
            FileSystem::FILE_SHARE_MODE(0),
            None,
            FileSystem::FILE_CREATION_DISPOSITION(FileSystem::CREATE_ALWAYS.0),
            FileSystem::FILE_FLAGS_AND_ATTRIBUTES(FileSystem::FILE_ATTRIBUTE_NORMAL.0),
            HANDLE::default()
        ) {
            Ok(handle) => {
                println!("Got TOC handle!");
                /* fileemu_utoc_stream_emulator::toc_factory::add_from_folders(&test_mod_1);
                add_from_first_folder = start_making_toc.elapsed().as_micros();
                //fileemu_utoc_stream_emulator::add_from_folders(&test_mod_2);
                //add_from_second_folder = start_making_toc.elapsed().as_micros() - add_from_first_folder;
                fileemu_utoc_stream_emulator::toc_factory::build_table_of_contents(handle.0, &unreal_essentials_toc, &unreal_essentials_partition);
                build_toc = start_making_toc.elapsed().as_micros() - add_from_second_folder;
                */
            }
            Err(e) => println!("Error occurred trying to open file: {}", e.to_string())
        }
    }
    */
    /* 
    println!("Added files from first mod in {} ms", add_from_first_folder as f64 / 1000f64);
    println!("Added files from second mod in {} ms", add_from_second_folder as f64 / 1000f64);
    println!("Built Table of Contents in {} ms", build_toc as f64 / 1000f64); // This section is slow...
    println!("Total: {} ms", (add_from_first_folder + add_from_second_folder + build_toc) as f64 / 1000f64);
    */

}

/* Unit tests for cooked package to IO Store package conversion
// WIP
#[cfg(test)]
mod tests {
    use byteorder::{ReadBytesExt, WriteBytesExt};
    use crate::{
        io_package::{IoStoreObjectIndex, ObjectImport, ObjectExport2},
        io_toc::{IoStoreToc, IoStoreTocVersion},
        pak_package::{FObjectImport, FObjectExport, NameMap, NameMapImpl},
        partition::{GameName, GameNameImpl},
        string::{FStringDeserializer, FStringSerializer, FStringSerializerText, FString16, FString32},
        toc_factory,
    };
    use std::{
        fs, fs::File,
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
    // Tests for converting cooked packages to IO store packages
    // Current process without this would require packaging with IO Store, then using UnZen to extract packages
    #[test]
    fn convert_name_map_texture() {
        // Converts a name map for a PAK package into an IO Store name map. FString16 names are written first, followed by hashes
        let mut reader = Cursor::new(fs::read("test_resources/name_map_texture_in.bin").unwrap()); // Name map block from T_Chair_M.uasset
        let name_map = NameMapImpl::new_from_buffer::<CV, FString32, NE>(&mut reader, 15);
        let mut writer = Cursor::new(vec![]);
        name_map.to_buffer_two_blocks::<CV, FString16, NE>(&mut writer); // two blocks for IO Store stream
        write_and_assert("name_map_texture", writer.into_inner());
    }

    #[test]
    fn convert_name_map_particle() {
        let mut reader = Cursor::new(fs::read("test_resources/name_map_particle_in.bin").unwrap()); // Name map from P_Explosion.uasset
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

        let mut name_map_reader = Cursor::new(fs::read("test_resources/name_map_texture_in.bin").unwrap());
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

    // Define an OsPath field and a TocPath field

    fn collect_asset_files(mount: &str, path: &str) -> Vec<String> {
        let mut files: Vec<String> = vec![];
        for i in fs::read_dir(path).unwrap() {
            let curr_file = i.unwrap();
            let file_type = curr_file.file_type().unwrap();
            let file_name = mount.to_owned() + "/" + &curr_file.file_name().into_string().unwrap();
            if file_type.is_file() {
                files.push(file_name);
            } else if file_type.is_dir() {
                let dir_name = curr_file.file_name().into_string().unwrap();
                let os_dir_path = path.to_owned() + "/" + &dir_name;
                let toc_dir_path = mount.to_owned() + "/" + &dir_name;
                println!("{}", os_dir_path);
                files.extend(collect_asset_files(&toc_dir_path, &os_dir_path));
            }
        }
        files
    }
}
*/
