CREATE DATABASE dalan_fixture;
CREATE USER 'dalan_reader'@'%' IDENTIFIED BY 'dalan-local-fixture-only';
GRANT SELECT, SHOW VIEW ON dalan_fixture.* TO 'dalan_reader'@'%';
USE dalan_fixture;
CREATE TABLE contact (
    id BIGINT UNSIGNED PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(200) NULL,
    payload JSON,
    price DECIMAL(38,18),
    big_number BIGINT UNSIGNED,
    raw_data VARBINARY(32)
);
INSERT INTO contact VALUES
(1, 'Alice', 'alice@example.com', '{"active":true}', 12.345678901234567890, 18446744073709551615, X'414200FF'),
(2, 'Bob', NULL, '{"active":false}', 0.000000000000000001, 9007199254740993, X'00'),
(3, '100%_literal!', 'third@example.com', '{"active":null}', 42, 3, X'');
CREATE VIEW contact_view AS SELECT id, name FROM contact;
