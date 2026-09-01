from pathlib import Path

import pandas as pd
from src.freshie import Freshie
from src.db import add_freshies
import datetime

def extract_freshies(filename: Path) -> int:
    excel = pd.read_excel(filename, header=1, skipfooter=1)
    freshies = []
    for _, row in excel.iterrows():
        freshies.append(Freshie(
            _id=0,
            first_name=row["VORNAME"],
            last_name=row["NACHNAME"],
            email=row["EMAIL"],
            date_added=str(datetime.datetime.today()),
            batch_id=None
        ))
    number_freshies_added = add_freshies(freshies)
    return number_freshies_added
    
# def add_test_freshie():
#     add_freshies([
#         Freshie(
#             first_name = "john1",
#             last_name="johnson1",
#             email="john@6",
#             date_added=str(datetime.datetime.today())
#         ),
#         Freshie(
#             first_name = "john1",
#             last_name="johnson1",
#             email="john@6",
#             date_added=str(datetime.datetime.today())
#         )
#     ])

