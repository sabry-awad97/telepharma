-- Add migration script here
CREATE TABLE IF NOT EXISTS medicines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    stock INTEGER NOT NULL,
    expiry_date DATE NOT NULL
);

CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    medicine_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    status TEXT NOT NULL,
    created_at DATE NOT NULL,
    FOREIGN KEY (medicine_id) REFERENCES medicines(id)
);

-- Seed data for medicines table
INSERT OR IGNORE INTO medicines (name, stock, expiry_date) VALUES
    ('Acetaminophen 500mg', 1000, '2025-12-31'),
    ('Ibuprofen 200mg', 800, '2026-06-30'),
    ('Amoxicillin 250mg', 500, '2025-09-15'),
    ('Lisinopril 10mg', 600, '2026-03-31'),
    ('Metformin 500mg', 750, '2025-11-30'),
    ('Levothyroxine 50mcg', 400, '2026-08-31'),
    ('Amlodipine 5mg', 550, '2026-01-31'),
    ('Omeprazole 20mg', 700, '2025-10-31'),
    ('Sertraline 50mg', 450, '2026-04-30'),
    ('Atorvastatin 20mg', 600, '2026-02-28'),
    ('Metoprolol 25mg', 500, '2025-12-15'),
    ('Gabapentin 300mg', 350, '2026-05-31'),
    ('Escitalopram 10mg', 400, '2026-07-31'),
    ('Losartan 50mg', 550, '2026-03-15'),
    ('Albuterol Inhaler 90mcg', 200, '2025-11-30'),
    ('Hydrocodone/APAP 5-325mg', 300, '2025-09-30'),
    ('Metformin ER 750mg', 450, '2026-01-15'),
    ('Pantoprazole 40mg', 500, '2026-04-15'),
    ('Citalopram 20mg', 400, '2025-12-31'),
    ('Fluoxetine 20mg', 350, '2026-06-15');
