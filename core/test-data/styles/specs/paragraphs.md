# Acceptance Criteria (sdd-workshop style)

**PAR-1.1** Happy path
WHEN the input directory has CSV files
THEN the batch job executes
SHALL read all `*.csv` files

**PAR-1.2** Multiple files
WHEN there are many files
SHALL process all of them in one run

**PAR-1.10** Ten sorts after two
SHALL sort numerically
