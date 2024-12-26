use directories::UserDirs;
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select, Text};
use lopdf::{dictionary, xobject, Document, Object, Stream};
use reqwest::Client;
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use tokio::sync::Semaphore;

struct Manga {
    id: String,
    title: String,
    description: String,
    author: String,
}

#[derive(Clone, Debug)]
struct Chapter {
    id: String,
    number: String,
    name: String,
}

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .user_agent("MangaDownloader/1.0")
        .build()
        .expect("Failed to build client");

    clear_screen();

    let mut base_url = "";

    let websites: Vec<&str> = vec!["Mangadex", "Other(doesnt exist yet)"];
    let website: &str = Select::new("What Manga Provider would you like to use?", websites)
        .prompt()
        .unwrap();

    match website {
        "Mangadex" => base_url = "https://api.mangadex.org",
        _ => {
            println!("over.... (TODO)")
        }
    }

    let manga_title = Text::new("Enter the manga title you want to search for:")
        .prompt()
        .expect("Input needed");
    let mangas = fetch_manga_by_title(base_url, &manga_title, &client).await;

    if let Some(selected_manga) = user_select_manga(&mangas) {
        let chapters = fetch_all_chapters(&selected_manga.id, base_url, &client).await;

        let selected_chapters = user_select_chapter(&chapters);

        download_chapters_to_pdf(selected_chapters, &selected_manga.title, &client).await;
    }
}

async fn download_chapters_to_pdf(
    chapters: Vec<Chapter>,
    manga_title: &str,
    client: &reqwest::Client,
) {
    let user_dirs = UserDirs::new().expect("Could not determine user directories");
    let downloads_dir = user_dirs
        .download_dir()
        .expect("Could not determine Downloads directory");

    let manga_dir = downloads_dir.join(manga_title.replace("/", "_").replace("\\", "_"));
    if let Err(e) = fs::create_dir_all(&manga_dir) {
        eprintln!("Failed to create directory {:?}: {}", manga_dir, e);
        return;
    }

    let multi_progress = MultiProgress::new();

    println!("Dont interrupt this process, I dont want to write error handling for that");

    let tasks: Vec<_> = chapters
        .into_iter()
        .map(|chapter| {
            let manga_dir = manga_dir.clone();
            let client = client.clone();
            let multi_progress = multi_progress.clone();

            tokio::spawn(async move {
                let pb = multi_progress.add(ProgressBar::new(100));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{msg} [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({percent}%)")
                        .expect("Invalid progress bar template")
                        .progress_chars("=>-"),
                );

                let pdf_path = manga_dir.join(format!("Chapter_{}.pdf", chapter.number));
                pb.set_message(format!("Chapter {}", chapter.number));

                let images = fetch_chapter_images_parallel(&chapter.id, &client).await;

                pb.set_length(images.len() as u64);

                let mut doc = Document::with_version("1.5");
                let pages_id = doc.new_object_id();
                let mut page_objects = Vec::new();

                for (i, image_path) in images.into_iter().enumerate() {
                    pb.set_position(i as u64);

                    let img_stream = xobject::image(&image_path)
                        .expect("Failed to create image stream from file");

                    let img_id = doc.add_object(img_stream);

                    let (width, height) = image::image_dimensions(&image_path)
                        .expect("Failed to get image dimensions");

                    let content = lopdf::content::Content {
                        operations: vec![
                            lopdf::content::Operation::new("q", vec![]),
                            lopdf::content::Operation::new(
                                "cm",
                                vec![
                                    (width as f32).into(),
                                    0.0.into(),
                                    0.0.into(),
                                    (height as f32).into(),
                                    0.0.into(),
                                    0.0.into(),
                                ],
                            ),
                            lopdf::content::Operation::new(
                                "Do",
                                vec![lopdf::Object::Name(b"Img1".to_vec())],
                            ),
                            lopdf::content::Operation::new("Q", vec![]),
                        ],
                    };

                    let content_stream = doc.add_object(Stream::new(
                        lopdf::dictionary! {},
                        content.encode().unwrap(),
                    ));

                    let resources_id = doc.add_object(dictionary! {
                        "XObject" => dictionary! {
                            "Img1" => img_id,
                        },
                    });

                    let page = dictionary! {
                        "Type" => "Page",
                        "Parent" => lopdf::Object::Reference(pages_id),
                        "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                        "Contents" => content_stream,
                        "Resources" => lopdf::Object::Reference(resources_id),
                    };

                    page_objects.push(doc.add_object(page));
                    fs::remove_file(&image_path).expect("Failed to delete image file");
                }

                pb.finish_with_message(format!("Finished Chapter {}", chapter.number));

                let pages_tree = dictionary! {
                    "Type" => "Pages",
                    "Kids" => page_objects.iter().map(|id| (*id).into()).collect::<Vec<_>>(),
                    "Count" => page_objects.len() as i32,
                };

                doc.objects.insert(pages_id, Object::Dictionary(pages_tree));

                let catalog = dictionary! {
                    "Type" => "Catalog",
                    "Pages" => lopdf::Object::Reference(pages_id),
                };

                let catalog_id = doc.add_object(catalog);
                doc.trailer.set("Root", catalog_id);

                doc.compress();
                doc.save(&pdf_path).expect("Failed to save PDF");
            })
        })
        .collect();

    for task in tasks {
        task.await.unwrap();
    }

    println!("Download Finished! Check: {}", &manga_dir.display());
}

async fn fetch_chapter_images_parallel(chapter_id: &str, client: &reqwest::Client) -> Vec<String> {
    let url = format!("https://api.mangadex.org/at-home/server/{}", chapter_id);
    let response = client
        .get(&url)
        .send()
        .await
        .expect("Failed to fetch chapter images metadata.");

    if response.status().is_success() {
        let data: Value = response.json().await.unwrap();
        let base_url = data["baseUrl"].as_str().unwrap_or("");
        let hash = data["chapter"]["hash"].as_str().unwrap_or("");
        let image_filenames: Vec<(usize, String)> = data["chapter"]["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
            .filter_map(|(index, image)| image.as_str().map(|s| (index, s.to_string())))
            .collect();

        let semaphore = Arc::new(Semaphore::new(10));
        let futures: FuturesUnordered<_> = image_filenames
            .iter()
            .map(|(index, filename)| {
                let semaphore = semaphore.clone();
                let client = client.clone();
                let base_url = base_url.to_string();
                let hash = hash.to_string();
                let filename = filename.clone();
                async move {
                    let _permit = semaphore.acquire_owned().await;
                    let image_url = format!("{}/data/{}/{}", base_url, hash, filename);
                    let response = client.get(&image_url).send().await.ok();

                    if let Some(response) = response {
                        if response.status().is_success() {
                            let bytes = response.bytes().await.ok();
                            if let Some(bytes) = bytes {
                                let path = format!("{:04}_{}", index, filename);
                                fs::write(&path, &bytes).expect("Failed to write image file");
                                return Some(path);
                            }
                        }
                    }
                    None
                }
            })
            .collect();

        let mut results: Vec<String> = futures
            .filter_map(|result| async move { result })
            .collect()
            .await;

        results.sort();
        results
    } else {
        println!("Error fetching chapter images metadata.");
        vec![]
    }
}

fn user_select_chapter(chapters: &[Chapter]) -> Vec<Chapter> {
    let mut options: Vec<(u32, String)> = chapters
        .iter()
        .filter_map(|ch| {
            if let Ok(number) = ch.number.parse::<u32>() {
                Some((
                    number,
                    format!(
                        "Chapter {}: {} (ID: {})",
                        ch.number,
                        ch.name,
                        ch.id.chars().take(8).collect::<String>()
                    ),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    options.sort_by_key(|(number, _)| *number);

    let selection = MultiSelect::new(
        "Select chapters to download",
        options.iter().map(|(_, label)| label.clone()).collect(),
    )
    .with_page_size(30)
    .prompt()
    .expect("Failed to select chapters");

    options
        .into_iter()
        .filter(|(_, label)| selection.contains(label))
        .map(|(number, _)| {
            chapters
                .iter()
                .find(|ch| ch.number.parse::<u32>().ok() == Some(number))
                .unwrap()
                .clone()
        })
        .collect()
}

async fn fetch_all_chapters(
    manga_id: &str,
    base_url: &str,
    client: &reqwest::Client,
) -> Vec<Chapter> {
    let url = format!("{}/manga/{}/feed", base_url, manga_id);
    let response = client
        .get(&url)
        .query(&[("translatedLanguage[]", "en")])
        .send()
        .await
        .expect("Failed to fetch chapters.");

    if response.status().is_success() {
        let data: Value = response.json().await.unwrap();
        let chapters: Vec<Chapter> = data["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|ch| Chapter {
                id: ch["id"].as_str().unwrap().to_string(),
                number: ch["attributes"]["chapter"]
                    .as_str()
                    .unwrap_or("N/A")
                    .to_string(),
                name: ch["attributes"]["title"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        if chapters.is_empty() {
            println!("No English chapters found for this manga.");
            std::process::exit(1);
        }

        chapters
    } else {
        println!("Error fetching chapters.");
        std::process::exit(1);
    }
}

async fn fetch_manga_by_title(base_url: &str, title: &str, client: &reqwest::Client) -> Vec<Manga> {
    let url = format!("{}/manga", base_url);
    let response = client
        .get(&url)
        .query(&[("title", title)])
        .send()
        .await
        .expect("Failed to fetch manga data.");

    if response.status().is_success() {
        let data: Value = response.json().await.unwrap();
        let mut mangas = Vec::new();

        for m in data["data"].as_array().unwrap_or(&vec![]) {
            let author_ids: Vec<String> = m["relationships"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter(|rel| rel["type"] == "author")
                .filter_map(|rel| rel["id"].as_str())
                .map(|id| id.to_string())
                .collect();

            let authors = fetch_author_names(&author_ids, base_url, client).await;

            mangas.push(Manga {
                id: m["id"].as_str().unwrap().to_string(),
                title: m["attributes"]["title"]["en"]
                    .as_str()
                    .unwrap_or("No title")
                    .to_string(),
                description: m["attributes"]["description"]["en"]
                    .as_str()
                    .unwrap_or("No description")
                    .to_string(),
                author: authors.join(", "),
            });
        }
        mangas
    } else {
        println!("Error fetching manga data.");
        vec![]
    }
}

async fn fetch_author_names(author_ids: &[String], base_url: &str, client: &Client) -> Vec<String> {
    let mut author_names = Vec::new();

    for author_id in author_ids {
        let url = format!("{}/author/{}", base_url, author_id);
        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                if let Ok(data) = response.json::<Value>().await {
                    if let Some(name) = data["data"]["attributes"]["name"].as_str() {
                        author_names.push(name.to_string());
                    }
                }
            }
        }
    }

    if author_names.is_empty() {
        author_names.push("Unknown author".to_string());
    }

    author_names
}

fn user_select_manga(mangas: &[Manga]) -> Option<&Manga> {
    let options: Vec<String> = mangas
        .iter()
        .map(|m| format!("========================================\n  Title: {} \n==========================================\n  Author: {}\n==========================================\n  Description: {}\n", m.title, m.author, m.description))
        .collect();
    let selection = Select::new("Select a manga", options)
        .prompt()
        .expect("Failed to select a manga");

    mangas.iter().find(|m| format!("========================================\n  Title: {} \n==========================================\n  Author: {}\n==========================================\n  Description: {}\n", m.title, m.author, m.description) == selection)
}

fn clear_screen() {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cls").status().unwrap();
    } else {
        std::process::Command::new("clear").status().unwrap();
    }
}
