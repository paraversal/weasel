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
        email TEXT NOT NULL,
        date_added TEXT NOT NULL,
        batch_id INTEGER NOT NULL,
        UNIQUE(email)
        );""",
        f"""--sql
        CREATE TABLE IF NOT EXISTS campaigns(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        text TEXT NOT NULL,
        info TEXT NOT NULL
        );""",
        f"""--sql
        CREATE TABLE IF NOT EXISTS pastes(
        id INTEGER PRIMARY KEY AUTOINCREMENT
        );""",
        f"""--sql
        CREATE TABLE IF NOT EXISTS campaigns_sent_instances(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        campaign_id INTEGER NOT NULL,
        student_id INTEGER NOT NULL,
        paste_id INTEGER NOT NULL,
        FOREIGN KEY(campaign_id) REFERENCES campaigns(id),
        FOREIGN KEY(student_id) REFERENCES freshies(id),
        FOREIGN KEY(paste_id) REFERENCES pastes(id)
        );"""
    ]
    for table in tables:
        cursor.execute(table)
    return connection

def add_freshies(freshies: list[Freshie]) -> int:
    """Add all freshies that do not exist yet to the db. Returns number of freshies added."""
    connection = db_ready()
    cursor = connection.cursor()
    count_added = 0

    next_batch_id = cursor.execute("SELECT COALESCE(MAX(batch_id), 0) FROM freshies").fetchone()[0] + 1
    
    for freshie in freshies:
        count_before = cursor.execute("SELECT COUNT(*) FROM freshies").fetchone()
        cursor.execute("""--sql
            INSERT OR IGNORE INTO freshies(
                first_name,
                last_name,
                email,
                date_added,
                batch_id
            )
            VALUES (?, ?, ?, ?, ?);
            """,
            (
                freshie.first_name,
                freshie.last_name,
                freshie.email,
                freshie.date_added,
                next_batch_id
            )
        )
        count_after = cursor.execute("SELECT COUNT(*) FROM freshies").fetchone()
        if count_after > count_before:
            count_added += 1

    connection.commit()
    connection.close()
    return count_added

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

def get_all_freshies():
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