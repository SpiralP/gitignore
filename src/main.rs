use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
  env::args,
  error::Error,
  io::{stdout, Write},
  path::Path,
};

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Serialize, Deserialize, Debug)]
struct TreesResponse {
  sha: String,
  // url: String,
  tree: Vec<Tree>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Tree {
  path: String,
  // mode: String,
  r#type: String,
  // sha: String,
  url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TreesResponseError {
  message: String,
  // documentation_url: String
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let client = Client::builder().user_agent(APP_USER_AGENT).build()?;

  let response = client
    .get("https://api.github.com/repos/github/gitignore/git/trees/master?recursive=1")
    .send()
    .await?;

  let bytes = response.bytes().await?;

  let err: serde_json::Result<TreesResponseError> = serde_json::from_slice(&bytes);
  let json: TreesResponse = match err {
    Ok(err) => {
      return Err(err.message.into());
    }

    Err(_) => serde_json::from_slice(&bytes)?,
  };

  let gitignore_trees: Vec<_> = json
    .tree
    .iter()
    .filter(|Tree { path, r#type, .. }| {
      if r#type != "blob" {
        return false;
      }

      let path = Path::new(&path);

      if !path
        .extension()
        .map(|extension| extension == "gitignore")
        .unwrap_or(false)
      {
        return false;
      }

      true
    })
    .collect();

  let mut found_trees = Vec::new();

  for arg in args().skip(1) {
    let tree = gitignore_trees
      .iter()
      .find(|Tree { path, .. }| {
        Path::new(path)
          .file_stem()
          .map(|file_stem| file_stem.to_string_lossy().to_lowercase() == arg.to_lowercase())
          .unwrap_or(false)
      })
      .ok_or_else(|| format!("couldn't find template for {}", arg))?;

    found_trees.push(tree);
  }

  for Tree { url, path, .. } in &found_trees {
    let blob_url = format!(
      "https://github.com/github/gitignore/blob/{}/{}",
      json.sha, path
    );

    let resp = client
      .get(url)
      .header("Accept", "application/vnd.github.v3.raw")
      .send()
      .await?;

    let bytes = resp.bytes().await?;
    let mut stdout = stdout();
    writeln!(stdout)?;
    writeln!(stdout, "# {}", blob_url)?;
    writeln!(stdout)?;

    // TODO \r\n??
    stdout.write_all(&bytes)?;

    if !bytes.ends_with(b"\n\n") {
      writeln!(stdout)?;
    }
    writeln!(
      stdout,
      "# End of {}",
      Path::new(path).file_name().unwrap().to_string_lossy()
    )?;
    writeln!(stdout)?;
  }

  if found_trees.is_empty() {
    eprintln!("No templates found!");
    let mut set: std::collections::HashSet<_> = gitignore_trees
      .iter()
      .filter_map(|Tree { path, .. }| Path::new(path).file_stem())
      .map(|file_stem| file_stem.to_string_lossy())
      .collect();
    let mut vec: Vec<_> = set.drain().collect();
    vec.sort();
    eprintln!("{}", vec.join(", "));
  }

  Ok(())
}
