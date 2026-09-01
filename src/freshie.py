class Freshie():
    def __init__(self, _id, first_name, last_name, email, date_added, batch_id):
        self.id: str = _id
        self.first_name: str = first_name
        self.last_name: str = last_name
        self.email: str = email
        self.date_added: str = date_added
        self.batch = batch_id
    
    def __repr__(self):
        return f"Freshie: {self.first_name} {self.last_name}"
    
def db_row_to_freshie(db_freshie: tuple):
    return Freshie(
            _id = db_freshie[0],
            first_name=db_freshie[1],
            last_name=db_freshie[2],
            email=db_freshie[3],
            date_added = db_freshie[4],
            batch_id = db_freshie[5]
        )

def db_rows_to_freshies(db_freshies: list[tuple]) -> list[Freshie]:
    return [Freshie(
            _id = db_freshie[0],
            first_name=db_freshie[1],
            last_name=db_freshie[2],
            email=db_freshie[3],
            date_added = db_freshie[4],
            batch_id = db_freshie[5]
        ) for db_freshie in db_freshies]