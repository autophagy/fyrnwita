CREATE TABLE quotes(id INTEGER primary key, quote text not null, submitter text, submitted datetime);
CREATE VIRTUAL TABLE quotes_fts using fts5(quote, content=quotes, content_rowid=id);
CREATE TRIGGER quotes_ai AFTER INSERT ON quotes BEGIN
 INSERT INTO quotes_fts(rowid, quote) VALUES (new.id, new.quote);
END;
CREATE TRIGGER quotes_ad AFTER DELETE ON quotes BEGIN
 INSERT INTO quotes_fts(quotes_fts, rowid, quote) VALUES('delete', old.id, old.quote);
END;
CREATE TRIGGER quotes_au AFTER UPDATE ON quotes BEGIN
 INSERT INTO quotes_fts(quotes_fts, rowid, quote) VALUES('delete', old.id, old.quote);
 INSERT INTO quotes_fts(rowid, quote) VALUES (new.id, new.quote);
END;
