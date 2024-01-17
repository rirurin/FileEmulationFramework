use bitflags::bitflags;

pub trait GameName {
    fn get_project_name(&self) -> &str;
    fn get_game_name(&self) -> &str;
    fn project_path_to_game_path(&self, path: &str) -> Result<String, &'static str>;
}

pub struct GameNameImpl {
    project_name: String, // the name given to your Unreal project
    game_name: String // "Project Name" in Editor > Project Settings > Description
}

impl GameNameImpl {
    pub fn new(project_name: &str, game_name: &str) -> Self {
        let project_name = String::from(project_name);
        let game_name = String::from(game_name);
        Self { project_name, game_name }
    }
}

impl GameName for GameNameImpl {
    fn get_project_name(&self) -> &str {
        &self.project_name
    }
    fn get_game_name(&self) -> &str {
        &self.game_name
    }
    fn project_path_to_game_path(&self, path: &str) -> Result<String, &'static str> {
        let path_match = String::from(&self.project_name) + "/Content";
        match path.rfind(&path_match) {
            Some(_) => Ok(String::from(path).replace(&path_match, &self.game_name)),
            None => Err("Couldn't convert the project path to game path")
        }
    }
}

pub struct AssetPath {
    // Cache results for project_path and game_path here so we don't need to process that each time
    // Filenames have their respective extension removed
    project_path: String,
    game_path: String
}

impl AssetPath {

}