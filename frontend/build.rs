use std::{path::Path, error::Error, fs::{create_dir, File, remove_dir_all}, env, io::{Write, Read}};

use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=style.scss");

    let out_dir = env::var("OUT_DIR")?;
    let out_path = Path::new(&out_dir);
    let frontend_build_dir = out_path.join("frontend-build");
    let blueprint_dir = frontend_build_dir.join("~@blueprintjs");

    if !frontend_build_dir.exists() {
        create_dir(&frontend_build_dir)?;
    }

    if blueprint_dir.exists() {
        remove_dir_all(&blueprint_dir)?;
    }
    create_dir(&blueprint_dir)?;

    

    // download_from_npm("https://registry.npmjs.org/@blueprintjs/core/-/core-4.16.3.tgz", Path::new("core"), &blueprint_dir)?;
    // download_from_npm("https://registry.npmjs.org/@blueprintjs/colors/-/colors-4.1.15.tgz", Path::new("colors"), &blueprint_dir)?;
    // download_from_npm("https://registry.npmjs.org/@blueprintjs/icons/-/icons-4.13.2.tgz", Path::new("icons"), &blueprint_dir)?;

    download_from_npm("https://registry.npmjs.org/@blueprintjs/core/-/core-3.54.0.tgz", Path::new("core"), &blueprint_dir)?;
    download_from_npm("https://registry.npmjs.org/@blueprintjs/colors/-/colors-3.0.0.tgz", Path::new("colors"), &blueprint_dir)?;
    download_from_npm("https://registry.npmjs.org/@blueprintjs/icons/-/icons-3.33.0.tgz", Path::new("icons"), &blueprint_dir)?;


    let archive_blueprint_path = Path::new("blueprint--blueprintjs-core-4.16.3");

    // let resp = ureq::get("https://github.com/palantir/blueprint/archive/refs/tags/@blueprintjs/core@4.16.3.tar.gz").call()?;
    let resp = ureq::get("https://github.com/palantir/blueprint/archive/refs/tags/@blueprintjs/core@3.54.0.tar.gz").call()?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(resp.into_reader()));

    for entry in archive.entries()?
        .filter(|entry| entry
            .as_ref()
            .ok()
            .and_then(|entry| entry.path().ok())
            .map(|path| path.starts_with(archive_blueprint_path.join("resources")))
            .unwrap_or(false)
        )
    {
        let mut entry = entry?;
        let path = blueprint_dir.join(entry.path()?.strip_prefix(archive_blueprint_path)?);
        entry.unpack(path)?;
    }

    for entry in WalkDir::new(&blueprint_dir)
        .into_iter()
        .filter_map(|file| file.ok())
        .filter(|file| file.file_type().is_file())
    {
        let mut s = String::new();
        if File::open(entry.path())?.read_to_string(&mut s).is_err() {
            continue;
        };
        if s.contains("svg-icon(\"") && !s.contains("svg-icon($") {
            // dbg!(entry.path());
            replace_svg_icon(&mut s, &blueprint_dir.join("resources/icons"))?;
        }
        File::create(entry.path())?.write_all(s.as_bytes())?;
    }

    let style_scss = Path::new("style.scss");
    let style_scss_build = frontend_build_dir.join(style_scss);
    std::fs::copy(style_scss, &style_scss_build)?;

    let static_dir = Path::new("static");
    if !static_dir.exists() {
        create_dir(static_dir)?;
    }

    // Don't write to style.css unles needed. Trunk picks it up as it having changed.
    // let style_css_contents = grass::from_path(
    //     style_scss_build,
    //     &grass::Options::default()
    //     .load_path(frontend_build_dir)
    //         .load_path(&blueprint_dir),
    // )?;
    let style_css_contents = sass_rs::compile_file(style_scss_build, sass_rs::Options {
        include_paths: vec![frontend_build_dir.to_string_lossy().to_string(), blueprint_dir.to_string_lossy().to_string()],
        ..Default::default()
    })?;

    let style_css = static_dir.join("style.css");
    match File::open(&style_css).and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
    }) {
        Ok(contents) if contents == style_css_contents => { },
        _ => File::create(style_css)?.write_all(style_css_contents.as_bytes())?,
    }

    let icon_dir = static_dir.join("icons");
    if !icon_dir.exists() {
        create_dir(&icon_dir)?;
    }

    let blueprint_icon_dir = blueprint_dir.join("resources/icons");
    
    for entry in WalkDir::new(&blueprint_icon_dir)
        .into_iter()
        .filter_map(|file| file.ok())
    {
        dbg!(entry.path());
        let new_path = icon_dir.join(entry.path().strip_prefix(&blueprint_icon_dir)?);
        dbg!(&new_path);
        if entry.file_type().is_dir() {
            if !new_path.exists() {
                create_dir(&new_path)?;
            }
        } else {
            std::fs::copy(entry.path(), new_path)?;
        }
    }

    // let css_path = Path::new("blueprint.css");

    // if !css_path.exists() {
    //     yewprint_css::download_css(css_path)?;
    // }

    Ok(())
}

fn download_from_npm(url: &str, package_name: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let resp = ureq::get(url).call()?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(resp.into_reader()));

    let archive_path = Path::new("package");

    // let dest = dest.join(archive_path);
    // if !dest.exists() {
    //     create_dir(&dest)?;
    // }

    for entry in archive.entries()?
        // .inspect(|entry| { dbg!(entry.as_ref().unwrap().path().unwrap()); })
        .filter(|entry| entry
            .as_ref()
            .ok()
            .and_then(|entry| entry.path().ok())
            .map(|path| path.starts_with(archive_path))
            .unwrap_or(false)
        )
    {
        let mut entry = entry?;
        // dbg!(entry.path()?);
        // let path = dest.join(entry.path()?.strip_prefix(archive_path)?);
        entry.unpack_in(dest)?;
    }

    std::fs::rename(dest.join(archive_path), dest.join(package_name))?;

    Ok(())
}

fn replace_svg_icon(s: &mut String, _svg_path: &Path) -> Result<(), Box<dyn Error>> {
    while let Some(replace_start) = s.find("svg-icon(\"") {
        let replace_end = replace_start + s[replace_start..].find(';').unwrap();
        let quote_start = replace_start + s[replace_start..].find('\"').unwrap();
        let quote_end = quote_start + s[quote_start + 1..].find('\"').unwrap() + 1;

        let path = &s[quote_start + 1..quote_end];
        // dbg!(replace_start, replace_end, quote_start, quote_end, path);
        // let mut svg = String::new();
        // File::open(svg_path.join(path))?
        //     .read_to_string(&mut svg)?;
        // let svg = urlencoding::encode(&svg);

        // *s = format!("{}url(\"data:image/svg+xml,{}\"){}", &s[..i], svg, &s[j..]);
        *s = format!("{}url(\"/static/icons/{}\"){}", &s[..replace_start], path, &s[replace_end..])
    }

    Ok(())
}