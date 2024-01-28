use std::{
    cell::RefCell,
    fs, fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::Instant
};
use crate::{
    io_package,
    platform::Metadata,
    toc_factory::TARGET_TOC
};

pub const FILE_EMULATION_FRAMEWORK_FOLDER:  &'static str = "FEmulator";
pub const EMULATOR_NAME:                    &'static str = "UTOC";
pub const PROJECT_NAME:                     &'static str = "UnrealEssentials";

// Root TOC directory (needs to be global)
pub static mut ROOT_DIRECTORY: Option<Rc<TocDirectory>> = None;
pub static mut ASSET_COLLECTOR_PROFILER: Option<AssetCollectorProfiler> = None;

// Create tree of assets that can be used to build a TOC
pub fn add_from_folders(mod_id: &str, mod_path: &str) {
    // Check profiler is active
    unsafe {
        if ASSET_COLLECTOR_PROFILER == None {
            ASSET_COLLECTOR_PROFILER = Some(AssetCollectorProfiler::new());
        }
    }
    let mut profiler_mod = AssetCollectorProfilerMod::new(mod_id, mod_path);
    let mod_path: PathBuf = [mod_path, FILE_EMULATION_FRAMEWORK_FOLDER, EMULATOR_NAME, TARGET_TOC].iter().collect();
    if Path::exists(Path::new(&mod_path)) {
        // Mutating a global variable is UB in a multithreaded context
        // Yes the compiler will complain about this
        // https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable
        unsafe {
            if let None = ROOT_DIRECTORY {
                ROOT_DIRECTORY = Some(Rc::new(TocDirectory::new(PROJECT_NAME))); // ProjectName
            }
            profiler_mod.valid_mod();
            add_from_folders_inner(Rc::clone(&ROOT_DIRECTORY.as_ref().unwrap()), &mod_path, profiler_mod.get_contents_mut());
        }
    }
    unsafe { ASSET_COLLECTOR_PROFILER.as_mut().unwrap().mods_loaded.push(profiler_mod); }
}

//      A <--------
//      ^    ^    ^
//      |    |    | (refs from child -> parent)
//      v    |    | (owns from parent -> child and in sibling and file linked lists)
//      B -> C -> D
pub struct TocDirectory {
    pub name:           String, // leaf name only (directory name or file name)
    pub parent:         RefCell<Weak<TocDirectory>>, // weakref to parent for path building for FIoChunkIds
    pub first_child:    RefCell<Option<Rc<TocDirectory>>>, // first child
    last_child:         RefCell<Weak<TocDirectory>>, // O(1) insertion on directory add
    pub next_sibling:   RefCell<Option<Rc<TocDirectory>>>, // next sibling
    pub first_file:     RefCell<Option<Rc<TocFile>>>, // begin file linked list, owns file children
    last_file:          RefCell<Weak<TocFile>>, // O(1) insertion on file add
}

impl TocDirectory {
    // constructor
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            parent: RefCell::new(Weak::new()),
            first_child: RefCell::new(None),
            last_child: RefCell::new(Weak::new()),
            next_sibling: RefCell::new(None), // root folder has no siblings
            first_file: RefCell::new(None),
            last_file: RefCell::new(Weak::new())
        }
    }
    // convenience function to create reference counted toc directories
    #[inline]
    pub fn new_rc(name: &str) -> Rc<Self> {
        Rc::new(TocDirectory::new(name))
    }
    // Returns true/false depending on if the target directory contains any child directories
    pub fn has_children(dir: Rc<TocDirectory>) -> bool {
        match *dir.first_child.borrow() {
            Some(_) => true,
            None => false
        }
    }
    // Returns true/false depending on if the target directory contains any child files
    pub fn has_files(dir: Rc<TocDirectory>) -> bool {
        match *dir.first_file.borrow() {
            Some(_) => true,
            None => false
        }
    }
    // Add a file child into directory that doesn't currently contain any other files
    #[inline]
    fn add_first_file(dir: Rc<TocDirectory>, file: Rc<TocFile>) {
        println!("ADD FIRST FILE ONTO {}: NAME {}", dir.name, file.name);
        *dir.first_file.borrow_mut() = Some(Rc::clone(&file));
        *dir.last_file.borrow_mut() = Rc::downgrade(&file);
    }
    // Replace an existing file in the file list. Kick it off the list so it drops on add_or_replace_file's scope
    #[inline]
    fn replace_file(
        dir: Rc<TocDirectory>, // containing directory
        prev_file: Option<Rc<TocFile>>, // previous file, which links to replacee (unless it's the *first* file)
        replacee: Rc<TocFile>, // like the "e" in bladee
        replacer: Rc<TocFile> // file that'll take the place of replacee in the chain
    ) {
        println!("REPLACE FILE IN {}: SWAP {} WITH {} (TODO)", dir.name, replacee.name, replacer.name);
    }
    // Add a file to the end of the directory's file list, which contains at least 1 existing file
    #[inline]
    fn add_another_file(dir: Rc<TocDirectory>, file: Rc<TocFile>) {
        println!("ADD ANOTHER FILE ONTO {}: NAME {}", dir.name, file.name);
        *dir.last_file.borrow().upgrade().unwrap().next.borrow_mut() = Some(Rc::clone(&file)); // own our new child on the end of children linked list
        *dir.last_file.borrow_mut() = Rc::downgrade(&file); // and set the tail to weakref of the new child
    }
    // go through file list to check if the target file already exists, then replace it with our own
    // otherwise, add our file to the end
    pub fn add_or_replace_file(dir: Rc<TocDirectory>, file: Rc<TocFile>) -> TocFileAddType {
        match TocDirectory::has_files(Rc::clone(&dir)) {
            true => { // :adachi_true: - search file linked list
                let mut found = false;
                let mut prev: Option<Rc<TocFile>> = None;
                let mut curr_file = Rc::clone(dir.first_file.borrow().as_ref().unwrap());
                loop {
                    if curr_file.name == file.name { // we got the file, replace it
                        found = true;
                        break
                    }
                    match Rc::clone(&curr_file).next.borrow().as_ref() { // check if next points to next entry in chain or ends the chain
                        Some(f) => {
                            prev = Some(Rc::clone(&curr_file));
                            curr_file = Rc::clone(&f);
                        },
                        None => { // couldn't find it to replace, add it to the end
                            break // we need to escape this scope to prevent creating mut ref of last_file->next while const ref last_file->next is still valid
                        }
                    }
                }
                if !found {
                    TocDirectory::add_another_file(Rc::clone(&dir), Rc::clone(&file));
                    TocFileAddType::Addition
                } else {
                    TocDirectory::replace_file(
                        Rc::clone(&dir),
                        prev, // prev is only set with second file in list onwards
                        Rc::clone(&curr_file),
                        Rc::clone(&file)
                    );
                    TocFileAddType::Replacement
                }
            },
            false => {
                TocDirectory::add_first_file(Rc::clone(&dir), Rc::clone(&file));
                TocFileAddType::Addition
            }
        }
    }
    // get a child directory from a parent directory if it exists
    // TODO: use a better search method (currently using a linear search)
    pub fn get_child_dir(parent: Rc<TocDirectory>, exist: &str) -> Option<Rc<TocDirectory>> {
        match TocDirectory::has_children(Rc::clone(&parent)) {
            true => {
                let mut curr_dir = Rc::clone(parent.first_child.borrow().as_ref().unwrap());
                let mut result = None;
                loop {
                    if curr_dir.name == exist { // we got our directory
                        result = Some(Rc::clone(&curr_dir));
                        break;
                    }
                    match Rc::clone(&curr_dir).next_sibling.borrow().as_ref() {
                        Some(ip) => curr_dir = Rc::clone(&ip),
                        None => break
                    }
                }
                result
            },
            false => None // has no children, can only not exist
        }
    }
    pub fn add_directory(parent: Rc<TocDirectory>, child: Rc<TocDirectory>) {
        *child.parent.borrow_mut() = Rc::downgrade(&parent); // set child node's parent as weak ref of parent
        println!("adding new directory child {}", child.name);
        if !TocDirectory::has_children(Rc::clone(&parent)) { // if parent has no nodes (if let doesn't work here since scope of &first_child extends to entire statement, overlapping &mut first_child)
            *parent.first_child.borrow_mut() = Some(Rc::clone(&child)); // head and tail set to new child
            *parent.last_child.borrow_mut() = Rc::downgrade(&child);
            return;
        }
        *parent.last_child.borrow().upgrade().unwrap().next_sibling.borrow_mut() = Some(Rc::clone(&child)); // own our new child on the end of children linked list
        *parent.last_child.borrow_mut() = Rc::downgrade(&child); // and set the tail to weakref of the new child
    }
}

#[derive(Debug, PartialEq)]
pub struct TocFile {
    pub next: RefCell<Option<Rc<TocFile>>>,
    pub name: String,
    pub file_size: u64,
    pub os_file_path: String // needed so we can open it, copy it then write it into partition
}

impl TocFile {
    // constructor
    fn new(name: &str, file_size: u64, os_path: &str) -> Self {
        Self {
            next: RefCell::new(None),
            name: String::from(name),
            file_size,
            os_file_path: String::from(os_path)
        }
    }
    // convenience function to create reference counted toc files
    #[inline]
    pub fn new_rc(name: &str, file_size: u64, os_path: &str) -> Rc<Self> {
        Rc::new(TocFile::new(name, file_size, os_path))
    }
}

pub enum TocFileAddType {
    Addition,
    Replacement
}

pub const SUITABLE_FILE_EXTENSIONS: &'static [&'static str] = ["uasset", "ubulk", "uptnl"].as_slice();
pub const MOUNT_POINT: &'static str = "../../../";

pub fn add_from_folders_inner(parent: Rc<TocDirectory>, os_path: &PathBuf, profiler: &mut AssetCollectorProfilerModContents) {
    // We've already checked that this path exists in AddFromFolders, so unwrap directly
    // This folder is equivalent to /[ProjectName]/Content, so our mount point will be
    // at least ../../../[ProjectName] (../../../Game/)
    // build an unsorted n-tree of directories and files, preorder traversal
    // higher priority mods should overwrite contents of files, but not directories
    //println!("add_from_folders2: {}", os_path.to_str().unwrap());
    for i in fs::read_dir(os_path).unwrap() {
        match &i {
            Ok(fs_obj) => { // we have our file system object, now determine if it's a directory or folder
                let fs_obj_os_name = fs_obj.file_name(); // this does assume that the object name is valid Unicode
                let name = String::from(fs_obj_os_name.to_str().unwrap()); // if it's not i'll be very surprised
                let file_type = fs_obj.file_type().unwrap();
                if file_type.is_dir() { // new directory. mods can only expand on this
                    let mut inner_path = PathBuf::from(os_path);
                    inner_path.push(&name);
                    match TocDirectory::get_child_dir(Rc::clone(&parent), &name) {
                        // check through folder regardless since there may be new inner folders in there
                        Some(child_dir) => add_from_folders_inner(Rc::clone(&child_dir), &inner_path, profiler),
                        None => {
                            // this is a new directory, create it and then check inside it
                            let new_dir = TocDirectory::new_rc(&name);
                            TocDirectory::add_directory(Rc::clone(&parent), Rc::clone(&new_dir));
                            add_from_folders_inner(Rc::clone(&new_dir), &inner_path, profiler);
                        }
                    }
                    profiler.add_directory();
                } else if file_type.is_file() {
                    // ignore .uexp, that will be combined in build_table_of_contents
                    match PathBuf::from(&name).extension() {
                        Some(ext) => {
                            let ext_str = ext.to_str().unwrap();
                            match SUITABLE_FILE_EXTENSIONS.iter().find(|exist| **exist == ext_str) {
                                // it's a matter of either replacing an existing file or adding a new file
                                // ,,,at least until we start thinking about merging P3RE persona tables (lol)
                                Some(io_ext) => {
                                    if *io_ext == "uasset" { // export bundles - requires checking file header to ensure that it doesn't have the cooked asset signature
                                        let current_file = File::open(fs_obj.path().to_str().unwrap()).unwrap();
                                        let mut file_reader = BufReader::with_capacity(4, current_file);
                                        if !io_package::is_valid_asset_type::<BufReader<File>, byteorder::NativeEndian>(&mut file_reader) {
                                            profiler.add_skipped_file(os_path.to_str().unwrap(), format!("Uses cooked package"));
                                            continue
                                        }
                                    }
                                    let new_file = TocFile::new_rc(&name, Metadata::get_file_size(fs_obj), fs_obj.path().to_str().unwrap());
                                    match TocDirectory::add_or_replace_file(Rc::clone(&parent), Rc::clone(&new_file)) {
                                        TocFileAddType::Addition => profiler.add_added_file(),
                                        TocFileAddType::Replacement => profiler.add_replaced_file()
                                    }
                                },
                                // TODO: Unsupported file extensions go into PAK
                                // Io Store forces you to also make a pak file (hopefully DC's patches can fix this)
                                None => profiler.add_skipped_file(fs_obj.path().to_str().unwrap(), format!("Unsupported file type"))
                            }
                        }
                        None => profiler.add_skipped_file(fs_obj.path().to_str().unwrap(), format!("No file extension"))
                    }
                }
            },
            Err(e) => profiler.add_failed_fs_object(os_path.to_str().unwrap(), e.to_string())
        }
    }
}

// Safety: this checks if ASSET_COLLECTOR_PROFILER has been assigned a value first, which only happens after loading a mod
pub unsafe fn print_asset_collector_results() {
    if ASSET_COLLECTOR_PROFILER == None {
        return;
    }
    ASSET_COLLECTOR_PROFILER.as_ref().unwrap().print()
}

#[derive(Debug, PartialEq)]
pub struct AssetCollectorProfilerFailedFsObject {
    os_path: String,
    reason: String
}

#[derive(Debug, PartialEq)]
pub struct AssetCollectorSkippedFileCount {
    os_path: String,
    reason: String,
}

#[derive(Debug, PartialEq)]
pub struct AssetCollectorProfilerModContents {
    failed_file_system_objects: Vec<AssetCollectorProfilerFailedFsObject>,
    directory_count: u64,
    added_files_count: u64,
    replaced_files_count: u64,
    incorrect_asset_header: Vec<String>,
    skipped_files: Vec<AssetCollectorSkippedFileCount>,
    timer: Instant,
    time_to_tree: u128,
}

impl AssetCollectorProfilerModContents {
    pub fn new() -> Self {
        Self {
            failed_file_system_objects: vec![],
            directory_count: 0,
            added_files_count: 0,
            replaced_files_count: 0,
            incorrect_asset_header: vec![],
            skipped_files: vec![],
            timer: Instant::now(),
            time_to_tree: 0,
        }
    }
    
    pub fn add_failed_fs_object(&mut self, parent_dir: &str, reason: String) {
        self.failed_file_system_objects.push(AssetCollectorProfilerFailedFsObject { os_path: parent_dir.to_owned(), reason })
    }

    pub fn add_skipped_file(&mut self, os_path: &str, reason: String) {
        self.skipped_files.push(AssetCollectorSkippedFileCount { os_path: os_path.to_owned(), reason })
    }
    pub fn add_directory(&mut self) {
        self.directory_count += 1;
    }
    pub fn add_added_file(&mut self) {
        self.added_files_count += 1;
    }
    pub fn add_replaced_file(&mut self) {
        self.replaced_files_count += 1;
    }
    pub fn get_tree_time(&mut self) {
        self.time_to_tree = self.timer.elapsed().as_micros();
    }

    pub fn print(&self) {
        println!("{} directories added", self.directory_count);
        println!("{} added files", self.added_files_count);
        println!("{} replaced files", self.replaced_files_count);
    }
}

#[derive(Debug, PartialEq)]
pub struct AssetCollectorProfilerMod {
    uid: String, // p3rpc.modname
    os_path: String,
    data: Option<AssetCollectorProfilerModContents>
}

impl AssetCollectorProfilerMod {
    pub fn new(mod_id: &str, mod_path: &str) -> Self {
        Self {
            uid: mod_id.to_owned(),
            os_path: mod_path.to_owned(),
            data: None
        }
    }

    pub fn valid_mod(&mut self) {
        self.data = Some(AssetCollectorProfilerModContents::new());
    }
    pub fn get_contents_mut(&mut self) -> &mut AssetCollectorProfilerModContents {
        self.data.as_mut().unwrap()
    }

    fn print(&self) {
        println!("Mod Id: {}", self.uid);
        println!("Mod Path: {}", self.os_path);
        match &self.data {
            Some(n) => {
                n.print()
            },
            None => println!("This mod's path doesn't exist")
        };
    }
}

#[derive(Debug, PartialEq)]
pub struct AssetCollectorProfiler {
    mods_loaded: Vec<AssetCollectorProfilerMod>
}

impl AssetCollectorProfiler {
    pub fn new() -> Self {
        Self {
            mods_loaded: vec![],
        }
    }
    pub fn print(&self) {
        let top_header = "=".repeat(80);
        println!("{}", draw_top_of_box("+", "=", 80));
        println!("Asset Collector Profiler Summary");
        println!("{} mods loaded", self.mods_loaded.len());
        for i in &self.mods_loaded {
            i.print();
        }
    }
}

pub fn draw_top_of_box(corner: &str, middle: &str, length: usize) -> String {
    format!("{}{}{}", corner, middle.repeat(length), corner)
}

/* 
pub fn write_centered_text(text: &str, length: usize) -> String {
    let space_len = length - text.len();
}
*/