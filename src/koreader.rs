use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};

pub fn change_progression_to_99_percent(f: &Path) -> Option<()> {
    let filename = f.file_stem()?.to_string_lossy();
    let lua_path = f
        .parent()?
        .join(format!("{filename}.sdr/metadata.epub.lua"));

    substitute(&lua_path, |s| {
        s.replace(
            r#"["percent_finished"] = 1"#,
            r#"["percent_finished"] = 0.99"#,
        )
    })
    .ok()
}

pub fn substitute<F>(file_path: &Path, substitution: F) -> Result<(), io::Error>
where
    F: Fn(&str) -> String,
{
    // Open and read the file entirely
    let data = fs::read_to_string(file_path)?;

    // Run the replace operation in memory
    let new_data = substitution(&data);

    // Recreate the file and dump the processed contents to it
    let mut dst = File::create(file_path)?;
    dst.write_all(new_data.as_bytes())?;
    Ok(())
}
