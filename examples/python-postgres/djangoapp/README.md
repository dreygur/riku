# djangoapp

The Django project package for the [python-postgres example](../README.md).

- `settings.py`: uses SQLite by default and switches to a Postgres database named after `RIKU_DOMAINS` when that variable is set; `ALLOWED_HOSTS` is also taken from `RIKU_DOMAINS`. Static files collect into `static/`.
- `urls.py`: routes only `admin/`.
- `wsgi.py`: the `application` object the Procfile's `wsgi:` entry serves.

`settings.py` ships with a hardcoded `SECRET_KEY` and `DEBUG = True`. Both are
fine for a demo and wrong for anything public: replace the key and turn debug
off before exposing a copy of this app.
