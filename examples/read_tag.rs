use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let tag = xattr::get("helper-scripts/sample.txt", "user.xdg.tags")?;
    let tag = String::from_utf8(tag.unwrap()).unwrap();
    println!("{tag}");
    Ok(())
}
