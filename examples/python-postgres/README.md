# Postgres-backed Python + Django App

This is a simple Riku app to demonstrate deploying a Postgres-backed Django app.

During the `release` worker phase this app creates a Postgres database, as well as running the Django `collectstatic` and `migrate` tasks. The `release` worker will use the domain name (`RIKU_DOMAINS`) for the database name and the Django app assumes this in [settings.py](pikudjango/settings.py), so make sure you set the config variable to specify a domain name. See below for instructions.

This example uses Django 6.1, which needs Python 3.12 or newer. The Python
runtime plugin builds the app with whatever `python3` the server provides, so on
a distro that ships an older Python (Debian 12 has 3.11) the build fails at
`pip install`. Debian 13 ships 3.13.

In order for this to work you will first need to install `postgresql` on your Riku server:

```bash
sudo apt install -y postgresql postgresql-contrib
sudo systemctl enable postgresql
sudo systemctl start postgresql
```

To publish this app to Riku, make a copy of this folder and run the following commands inside the copy:

```bash
git init .
git remote add riku riku@your_server:pypostgres
git add .
git commit -a -m "initial commit"
git push riku main
```

Then you can connect a domain, set up an SSL cert, and create a database by setting the `RIKU_DOMAINS` config variable:

```bash
riku config set pypostgres RIKU_DOMAINS=your_domain_name.com RIKU_HTTPS_ONLY=1
```

You can also create a superuser and set a password like this:

```bash
riku run pypostgres ./manage.py createsuperuser --email your@email.com --username admin --no-input
riku run pypostgres ./manage.py changepassword admin
```

You will not see a prompt after the second command but you can type a new password anyway and hit enter.
