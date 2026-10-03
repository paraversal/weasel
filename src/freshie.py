from dataclasses import dataclass


@dataclass
class Freshie():
    id: str
    first_name: str
    last_name: str
    email: str
    date_added: str
    batch: int
    removed: bool
    
    def __repr__(self):
        return f"Freshie: {self.first_name} {self.last_name}"
    
def db_row_to_freshie(db_freshie: tuple):
    return Freshie(
            id = db_freshie[0],
            first_name=db_freshie[1],
            last_name=db_freshie[2],
            email=db_freshie[3],
            date_added = db_freshie[4],
            batch = db_freshie[5],
            removed = db_freshie[6]
        )

def db_rows_to_freshies(db_freshies: list[tuple]) -> list[Freshie]:
    return [db_row_to_freshie(db_freshie) for db_freshie in db_freshies]