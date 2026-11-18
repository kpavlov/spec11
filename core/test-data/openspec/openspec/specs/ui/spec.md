# ui Specification

## Purpose
The browse page.

## Requirements

### Requirement: UIX-01 Serve the page
The system SHALL serve an HTML page at `GET /`.

#### Scenario: Page loads
- **WHEN** a user requests `GET /`
- **THEN** the response status is `200 OK`

### Requirement: UIX-02 Legacy export
The system SHALL export CSV.

### Requirement: Filter form without an ID
The page SHALL present a filter form.
