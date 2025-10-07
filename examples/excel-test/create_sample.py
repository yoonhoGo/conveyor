#!/usr/bin/env python3
"""
Create a sample Excel file for testing the Excel WASM plugin.
"""
import openpyxl
from openpyxl import Workbook

# Create workbook
wb = Workbook()
ws = wb.active
ws.title = "SampleData"

# Add headers
headers = ["ID", "Name", "Age", "City", "Salary"]
ws.append(headers)

# Add sample data
data = [
    [1, "Alice", 30, "New York", 75000.50],
    [2, "Bob", 25, "San Francisco", 85000.00],
    [3, "Charlie", 35, "Los Angeles", 65000.75],
    [4, "Diana", 28, "Seattle", 72000.00],
    [5, "Eve", 32, "Boston", 80000.25],
]

for row in data:
    ws.append(row)

# Save file
wb.save("sample.xlsx")
print("Created sample.xlsx with 5 data rows")
