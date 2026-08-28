class Freshie():
    def __init__(self, first_name, last_name, email, date_added):
        self.first_name: str = first_name
        self.last_name: str = last_name
        self.email: str = email
        self.date_added: str = date_added
    
    def __repr__(self):
        return f"Freshie: {self.first_name} {self.last_name}"
    
def db_row_to_freshie(db_freshie: tuple):
    return Freshie(
            first_name=db_freshie[1],
            last_name=db_freshie[2],
            email=db_freshie[3],
            date_added = db_freshie[4]
        )