use std::{error::Error, fs, io};
use reqwest::{header::USER_AGENT, blocking::Client};
use serde::Deserialize;
use clap::{Parser, Subcommand};
use whoami::Platform;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>
}

#[derive(Subcommand)]
enum Commands {
    Search {
        #[arg(short, long, help="The string to search for matching mods")]
        query: String,
        #[arg(short, long, help="Categories like \"optimization\", the modloader also goes here")]
        categories: Option<Vec<String>>,
        #[arg(short='v', long="gameversion", help="The Minecraft version to search mods for")]
        game_version: String
    },
    Download {
        #[arg(short, long, help="The project to download, can be a slug, e.g. \"sodium\", or a id, e.g. \"AABBCC\"")]
        project: String,
        #[arg(short = 'v', long, help="The targeted Minecraft version for the downloaded mod")]
        game_version: String,
        #[arg(short, long, help="The modloader for the mod")]
        loader: String,
        #[arg(long="mcdir", help="Use if you want to install the mod in the .minecraft\\mods folder")]
        minecraft_dir: bool
    },
    Info {
        #[arg(short, long, help="The project to get the desc of, can be a slug or an id")]
        project: String,
    },
    Dependencies {
        #[arg(short, long, help="The targeted project for getting the dependencies")]
        project: String,
        #[arg(short = 'v', long, help="The Minecraft version of the targeted mod")]
        game_version: String,
        #[arg(short, long, help="The loader of the targeted mod")]
        loader: String,
    }
}

#[derive(Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<Hit>
}

#[derive(Deserialize)]
struct ProjectResponse {
    body: String,
    categories: Vec<String>,
    title: String,
    project_type: String,
    slug: String,
}

#[derive(Deserialize)]
struct Hit {
    slug: String,
    title: String,
    description: String,
}

#[derive(Deserialize)]
struct ProjectVersion {
     dependencies: Vec<ProjectDependency>
}

#[derive(Deserialize, Clone)]
struct ProjectDependency {
    project_id: String,
    dependency_type: String,
}

#[derive(Deserialize)]
struct GameVersion {
    loaders: Vec<String>,
    files: Vec<GameFiles>,
}

#[derive(Deserialize, Clone)]
struct GameFiles {
    url: String,
    filename: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    if check_for_mods_dir().is_empty() {
        println!("Couldn't find the .mincraft directory, mods won't be installed there even if asked for");
    }
    let client = Client::new();

    let cli = Cli::parse();
    let _ = match &cli.command {
        Some(Commands::Search { query, categories, game_version }) => {
            search_mods(query, game_version, categories.as_ref().unwrap().to_vec(), &client)
        },

        Some(Commands::Download { project, game_version, loader, minecraft_dir }) => {
            let game_files = get_download_link(project, loader, game_version, &client);
            download_jar(game_files.unwrap(), &client, *minecraft_dir)
        },

        Some(Commands::Info { project }) => {
            project_info(project, &client)
        },

        Some(Commands::Dependencies { project, game_version, loader }) => {
            let _ = project_dependencies(project, loader, game_version, &client);
            Ok(())
        }

        _ => {
            println!("no command found");
            Ok(())
        }
    };
    Ok(())
}

fn adapt_to_facet(categories: Vec<String>, game_version: &str) -> String {
    let mut facet = "&facets=[[\"project_type=mod\"],".to_string();
    let facet_1 = "[\"";
    let facet_2 = "\"],";
    for element in categories.iter() {
        let filter = format!("{}categories:{}{}", facet_1, element, facet_2);
        facet.push_str(&filter);
    }
    if !game_version.is_empty() {
        let version = format!("{}versions:{}{}", facet_1, game_version, facet_2);
        facet.push_str(&version);
    }
    facet.push_str(facet_2);
    facet = remove_last_char(&facet, ',');
    facet = remove_last_char(&facet, '"');
    remove_last_char(&facet, ',')
}

fn search_mods(query: &str, game_version: &str, categories: Vec<String>, client: &Client) -> Result<(), Box<dyn Error>> {
    let facet = {
        let vec_buff: Vec<String> = categories.to_vec();
        if !vec_buff.is_empty() {
            adapt_to_facet(vec_buff, game_version)
        } else {
            String::new()
        }

    };
    let search_link = format!("https://api.modrinth.com/v2/search?query={}{}", query.trim(), facet);
    let resp = client.get(search_link).header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let resp_txt = resp.text()?;
    let processed_response: Result<ModrinthSearchResponse, _> = serde_json::from_str(&resp_txt);

    let hits = match processed_response {
        Ok(modrinth_response) => modrinth_response.hits,
        Err(e) =>{ 
            println!("Couldn't find any mods matching the query");
            return Err(Box::new(e))
        }
    };

    for hit in hits.iter() {
        println!("\"{}\" : {}", hit.title, hit.slug);
        println!("{}\n", hit.description);
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "No matching GameFiles found")))
}

fn project_dependencies(project: &str, loader: &str, game_version: &str, client: &Client) -> Result<(), Box<dyn Error>> {
    let resp = client.get(format!("https://api.modrinth.com/v2/project/{}/version?loader=[\"{}\"]&game_versions=[\"{}\"]", project, loader, game_version))
        .header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let resp_txt = resp.text()?;
    let processed_resp: Result<Vec<ProjectVersion>, serde_json::Error> = serde_json::from_str(&resp_txt);
    match processed_resp {
        Ok(prj_versions) => {
            let first_prj_v = prj_versions.first().unwrap();
            if !first_prj_v.dependencies.is_empty(){
                for dependency in first_prj_v.dependencies.iter() {
                    let dependency_project = get_project(&dependency.project_id, client).unwrap();
                    println!("{}: \"{}\" - {}", dependency.dependency_type, dependency_project.title, dependency_project.slug)
                };
            } else {
                println!("No dependencies found on this project's version");
                return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "No dependencies found for this project's version")));
            }
        },
        Err(e) => {
            println!("{}", e);
            return Err(Box::new(e))
        }
    };
    Ok(())
}

fn project_info(project: &str, client: &Client) -> Result<(), Box<dyn Error>> {
    let resp = client.get(format!("https://api.modrinth.com/v2/project/{}", project))
        .header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let processed_resp: Result<ProjectResponse, serde_json::Error> = serde_json::from_str(&resp.text()?);

    let project = match processed_resp {
        Ok(prj) => prj,
        Err(e) => {
            println!("{}", e);
            return Err(Box::new(e));
        }
    };
    
    println!("{} - {}", project.project_type, project.title);
    for category in project.categories.iter() {
        print!("{}", category);
    }
    println!("\n\n{}\n", project.body);

    Ok(())
}

fn get_download_link(slug: &str, loader: &str, game_version: &str, client: &Client) -> Result<GameFiles, Box<dyn Error>> {
    let download_link = format!("https://api.modrinth.com/v2/project/{}/version?loader=[\"{}\"]&game_versions=[\"{}\"]", slug, loader, game_version);
    let resp = client.get(&download_link).header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let resp_txt = resp.text()?;
    let processed_response: Vec<GameVersion> = match serde_json::from_str(&resp_txt) {
        Ok(response) => response,
        Err(e) => {
            println!("{}", e);
            return Err(Box::new(e))
        }
    };
    for version in processed_response {
        if version.loaders.contains(&loader.to_string()) {
            return Ok(version.files.first().unwrap().clone());
        }
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "No matching GameFiles found")))
}

fn download_jar(game_files: GameFiles, client: &Client, mcdir: bool) -> Result<(), Box<dyn Error>>{
    println!("Downloading {} from {}", game_files.filename, game_files.url);
    let resp = client.get(&game_files.url).header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    if resp.status().is_success() {
        let bytes = resp.bytes();
        if !check_for_mods_dir().is_empty() && mcdir {
                let _ = fs::write(format!("{}/{}", check_for_mods_dir(), game_files.filename), bytes?.as_ref());
        } else {
            let _ = fs::write(game_files.filename, bytes?.as_ref());
        }
    } else {
        println!("Couldn't get file from {}", &game_files.url);
    }
    Ok(())
}

fn get_project(project_id: &str, client: &Client) -> Result<ProjectResponse, Box<dyn Error>> {
    let resp = client.get(format!("https://api.modrinth.com/v2/project/{}", project_id)).header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let resp_txt = resp.text()?;
    let processed_response: Result<ProjectResponse, _> = serde_json::from_str(&resp_txt);
    let project = match processed_response {
        Ok(prj) => prj,
        Err(e) => {
            println!("Couldn't get the project");
            return Err(Box::new(e));
        }
    };
    Ok(project)
}

fn remove_last_char(string: &str, char: char) -> String {
    if let Some(index) = string.rfind(char) {
        let mut result = String::with_capacity(string.len() - 1);
        result.push_str(&string[..index]);
        result.push_str(&string[index + char.len_utf8()..]);
        result
    } else {
        string.to_string()
    }
}

fn check_for_mods_dir() -> String{
    let user = whoami::username();
    let platform = whoami::platform();
    match platform {
        Platform::Windows => {
            if fs::metadata(format!("C:/Users/{}/AppData/Roaming/.minecraft", user)).is_ok() {
                if fs::metadata(format!("C:/Users/{}/AppData/Roaming/.minecraft/mods", user)).is_ok() {
                    return String::from(format!("C:/Users/{}/AppData/Roaming/.minecraft/mods", user));
                } else {
                    let _ = fs::create_dir(format!("C:/Users/{}/AppData/Roaming/.minecraft/mods", user));
                    return format!("C:/Users/{}/AppData/Roaming/.minecraft/mods", user).to_string();
                }
            }
        },
        Platform::Linux => {
            if fs::metadata("~/.minecraft").is_ok() {
                if fs::metadata("~/.minecraft/mods").is_ok() {
                    return String::from("~/.minecraft/mods")
                } else {
                    let _ = fs::create_dir("~/.minecraft/mods");
                    return "~/.minecraft/mods".to_string();
                }
            }
        }
        _ => return "".to_string()
    }
    "".to_string()
}
