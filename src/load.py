import pandas as pd
from src.freshie import Freshie
from src.db import add_freshies
import datetime

def extract_freshies(filename: str) -> int:
    excel = pd.read_excel(filename, header=1, skipfooter=1)
    freshies = []
    for _, row in excel.iterrows():
        freshies.append(Freshie(
            first_name=row["VORNAME"],
            last_name=row["NACHNAME"],
            email=row["EMAIL"],
            program=row["ABSCHLUSS"],
            added_manually=False,
            date_added=str(datetime.datetime.today())
        ))
    add_freshies(freshies)
    return len(freshies)
    

