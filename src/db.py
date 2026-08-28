import sqlite3
from src.freshie import Freshie, db_row_to_freshie
from src.campaign import Campaign

DB_PATH = "weasel_lair.db"

def db_ready() -> sqlite3.Connection:
    connection = sqlite3.connect(DB_PATH)
    cursor = connection.cursor()
    tables = [
        f"""--sql 
        CREATE TABLE IF NOT EXISTS infos(
        key TEXT NOT NULL PRIMARY KEY,
        value,
        UNIQUE(key)
        );
        """,
        f"""--sql 
        CREATE TABLE IF NOT EXISTS freshies(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        first_name TEXT NOT NULL,
        last_name TEXT NOT NULL,
        country_origin TEXT,
        email TEXT NOT NULL,
        program TEXT NOT NULL,
        added_manually BOOLEAN NOT NULL,
        date_added TEXT NOT NULL
        );""",
        f"""--sql
        CREATE TABLE IF NOT EXISTS campaigns(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        text TEXT NOT NULL,
        info TEXT NOT NULL
        );""",
        f"""--sql
        CREATE TABLE IF NOT EXISTS campaigns_sent_instances(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        campaign_id INTEGER NOT NULL,
        student_id INTEGER NOT NULL,
        FOREIGN KEY(campaign_id) REFERENCES campaigns(id),
        FOREIGN KEY(student_id) REFERENCES freshies(id)
        );"""
    ]
    for table in tables:
        cursor.execute(table)
    return connection

def add_freshies(freshies: list[Freshie]):
    connection = db_ready()
    cursor = connection.cursor()
    for freshie in freshies:
        cursor.execute("""--sql
            INSERT INTO freshies(
                first_name,
                last_name,
                country_origin,
                email,
                program,
                added_manually,
                date_added
            )
            VALUES (?, ?, ?, ?, ?, ?, ?);
            """,
            (
                freshie.first_name,
                freshie.last_name,
                freshie.country_origin,
                freshie.email,
                freshie.program,
                freshie.added_manually,
                freshie.date_added
            )
        )

    connection.commit()
    connection.close()

def delete_freshie(freshie_id: int):
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute("""--sql
        DELETE FROM freshies
        WHERE id = ?;
        """,
        (
            freshie_id,
        )
    )
    
    connection.commit()
    connection.close()

def get_freshies():
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute("""--sql
        SELECT * FROM freshies;
        """)
    db_freshies = cursor.fetchall()
    freshies = []
    for db_freshie in db_freshies:
        freshies.append(db_row_to_freshie(db_freshie))
    connection.close()
    return freshies

def get_freshie(freshie_id: int):
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute("""--sql
        SELECT * FROM freshies
        WHERE id = ?;
        """,
        (
            freshie_id,
        )
    )
    row = cursor.fetchone()
    connection.close()
    return row

def update_freshie(freshie_id: int, freshie: Freshie):
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute("""--sql
        UPDATE freshies
        SET first_name = ?,
            last_name = ?,
            country_origin = ?,
            email = ?,
            program = ?,
            added_manually = ?,
            date_added = ?
        WHERE id = ?;
        """,
        (
            freshie.first_name,
            freshie.last_name,
            freshie.country_origin,
            freshie.email,
            freshie.program,
            freshie.added_manually,
            freshie.date_added,
            freshie_id,
        )
    )
    connection.close()

def get_all_campaigns():
    connection = db_ready()
    cursor = connection.cursor()
    campaigns: list[Campaign] = []
    for campaign in cursor.execute("SELECT * FROM campaigns;").fetchall():
        sent_to = cursor.execute("SELECT student_id FROM campaigns_sent_instances WHERE campaign_id = ?;", (campaign[0],)).fetchall()
        campaigns.append(Campaign(
            _id=campaign[0],
            name=campaign[1],
            text=campaign[2],
            info=campaign[3],
            sent_to=sent_to
        ))
    connection.close()
    return campaigns

def DEBUG_clear_table(name: str):
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute(f"DELETE FROM {name}")
    connection.commit()
    connection.close()
    
def get_campaign_sent_info(campaign_id: int) -> tuple[list[Freshie], list[Freshie]]:
    connection = db_ready()
    cursor = connection.cursor()
    sent = cursor.execute(
        """--sql
        SELECT *
        FROM campaigns_sent_instances c 
        JOIN freshies f ON student_id = f.id
        WHERE c.campaign_id = ?
        """,
        (str(campaign_id),)
    ).fetchall()

    sent_freshies = []
    for db_freshie in sent:
        sent.append(db_row_to_freshie(db_freshie))
    
    not_sent = cursor.execute(
        """--sql
        SELECT *
        FROM freshies f
        LEFT JOIN campaigns_sent_instances s 
        ON s.campaign_id = ? AND student_id = f.id
        WHERE s.id IS NULL
        """,
        (str(campaign_id),)
    )
    
    not_sent_freshies = []
    for db_freshie in not_sent:
        not_sent_freshies.append(db_row_to_freshie(db_freshie))
    
    return sent_freshies, not_sent_freshies

def add_campaign(name: str, text: str, info: str):
    connection = db_ready()
    cursor = connection.cursor()
    cursor.execute("""--sql
        INSERT INTO campaigns(
            name,
            text,
            info
        )
        VALUES (?, ?, ?);
        """,
        (
            name,
            text,
            info
        )
    )
    connection.commit()
    connection.close()