class Freshie():
    def __init__(self, first_name, last_name, email, program, added_manually, date_added):
        self.first_name: str = first_name
        self.last_name: str = last_name
        self.country_origin: str = "unknown"
        self.email: str = email
        self.program: str = program
        self.added_manually: bool = added_manually
        self.date_added: str = date_added
    
    def __repr__(self):
        return f"Freshie: {self.first_name} {self.last_name}"
    
def db_row_to_freshie(db_freshie: tuple):
    return Freshie(
            first_name=db_freshie[1],
            last_name=db_freshie[2],
            email=db_freshie[4],
            program=db_freshie[5],
            added_manually=db_freshie[6],
            date_added = db_freshie[7]
        )