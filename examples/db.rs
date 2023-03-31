use rusqlite::{Connection, Result};

#[derive(Debug)]
struct Person {
    id: u64,
    name: String,
    data: Option<Vec<u8>>,
}

fn main() -> Result<()> {
    let conn = Connection::open("./temp.db3")?;

    create_schema(&conn)?;
    let me = Person {
        id: 0,
        name: "Steven".to_string(),
        data: None,
    };
    conn.execute(
        "INSERT INTO person (name, data) VALUES (?1, ?2)",
        (&me.name, &me.data),
    )?;

    let mut stmt = conn.prepare("SELECT id, name, data FROM person")?;
    let person_iter = stmt.query_map([], |row| {
        Ok(Person {
            id: row.get(0)?,
            name: row.get(1)?,
            data: row.get(2)?,
        })
    })?;

    for person in person_iter {
        println!("Found person {:?}", person.unwrap());
    }
    Ok(())
}

fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE "files" (
            "path"	TEXT NOT NULL UNIQUE
        );
        CREATE TABLE "tags" (
            "tag"	TEXT NOT NULL,
            "file_id"	INTEGER NOT NULL,
            CONSTRAINT "tags_unique_entries" UNIQUE("file_id","tag"),
            FOREIGN KEY("file_id") REFERENCES "files"("id") ON DELETE CASCADE
        );"#,
        (),
    )?;

    Ok(())
}
