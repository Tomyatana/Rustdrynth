use std::{error::Error, fs::File, io::{self, Write}};
use reqwest::{header::USER_AGENT, blocking::Client};
use serde::Deserialize;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>
}

#[derive(Subcommand)]
enum Commands {
    Search {
        #[arg(short, long)]
        query: String,
        #[arg(short, long)]
        categories: Option<Vec<String>>,
        #[arg(short='v', long="gameversion")]
        game_version: String
    },
    Download {
        #[arg(short, long)]
        project: String,
        #[arg(short = 'v', long)]
        game_version: String,
        #[arg(short, long)]
        loader: String 
    },
    Info {
        #[arg(short, long)]
        project: String,
    },
    Dependencies {
        #[arg(short, long)]
        project: String,
        #[arg(short = 'v', long)]
        game_version: String,
        #[arg(short, long)]
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
struct ProjectVersions {
     loaders: Vec<String>,
     project_id: Vec<String>,
     dependencies: Vec<ProjectDependency>
}

#[derive(Deserialize)]
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
    let client = Client::new();

    let cli = Cli::parse();
    let _ = match &cli.command {
        Some(Commands::Search { query, categories, game_version }) => {
            search_mods(query, game_version, categories.as_ref().unwrap().to_vec(), &client)
        },

        Some(Commands::Download { project, game_version, loader }) => {
            let game_files = get_download_link(project, loader, game_version, &client);
            download_jar(game_files.unwrap(), &client)
        },

        Some(Commands::Info { project }) => {
            project_info(project, &client)
        },

        Some(Commands::Dependencies { project, game_version, loader }) => {
            let dependencies = project_dependencies(project, loader, game_version, &client);
            let processed_dependencies = match dependencies {
                Ok(dep) => dep,
                Err(e) => {
                    println!("{}", e);
                    return Err(e);
                }
            };
            for dependency in processed_dependencies.iter() {
                println!("{}, {}", dependency.dependency_type, get_project(&dependency.project_id, &client).unwrap().title);
            };
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

fn project_dependencies(project: &str, loader: &str, game_version: &str, client: &Client) -> Result<Vec<ProjectDependency>, Box<dyn Error>> {
    let resp = client.get(format!("https://api.modrinth.com/v2/project/{}/version?loader=[\"{}\"]&game_versions=[\"{}\"]", project, loader, game_version))
        .header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    let processed_resp: Result<ProjectVersions, serde_json::Error> = serde_json::from_str(&resp.text()?);
    match processed_resp {
        Ok(prj_versions) => {
            if !prj_versions.dependencies.is_empty(){
                for dependency in prj_versions.dependencies.iter() {
                    let dependency_project = get_project(&dependency.project_id, client).unwrap();
                    print!("\"{}\" - {}", dependency_project.title, dependency_project.slug)
                }
                return Ok(prj_versions.dependencies);
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

fn download_jar(game_files: GameFiles, client: &Client) -> Result<(), Box<dyn Error>>{
    let resp = client.get(game_files.url).header(USER_AGENT, "https://github.com/Tomyatana/Pydrinth/tree/Rustdrynth").send()?;
    if resp.status().is_success() {
        let bytes = resp.bytes();
        let jar = File::create(game_files.filename);
        let _ = jar.expect("Couldn't download file").write_all(bytes?.as_ref());
    } else {
        println!("Failed to download file");
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
