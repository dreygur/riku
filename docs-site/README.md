# Riku Documentation Site

This directory contains the MkDocs-based documentation site for Riku.

## Quick Start

### 1. Install Dependencies

```bash
# Create virtual environment (optional but recommended)
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt
```

### 2. Preview Locally

```bash
# Start development server with live reload
mkdocs serve

# Open http://localhost:8000 in your browser
```

### 3. Build Static Site

```bash
# Build for production
mkdocs build

# Output will be in the 'site/' directory
```

### 4. Deploy

Documentation is automatically deployed to GitHub Pages when you push to the `main` branch.

Manual deployment:
```bash
# Build and deploy using mkdocs-gh-deploy (optional)
pip install mkdocs-gh-deploy
mkdocs gh-deploy
```

---

## Directory Structure

```
docs-site/
├── docs/                  # Documentation source files
│   ├── index.md          # Home page
│   ├── runtimes/         # Runtime-specific docs
│   └── includes/         # Reusable content snippets
├── site/                  # Generated site (after build)
├── requirements.txt       # Python dependencies
└── mkdocs.yml            # MkDocs configuration (root directory)
```

---

## Writing Documentation

### File Format

All documentation is written in Markdown with MkDocs extensions:

```markdown
# Page Title

## Section

Content here...

=== "Tabbed Content"
    ```python
    print("Hello")
    ```

!!! note "Note Title"
    This is a note

!!! tip "Tip"
    This is a tip
```

### Admonitions

```markdown
!!! note "Note"
    Regular note

!!! tip "Tip"
    Helpful tip

!!! warning "Warning"
    Warning message

!!! danger "Danger"
    Danger message

!!! info "Info"
    Information
```

### Code Blocks

````markdown
```python
# Python code
def hello():
    print("Hello, World!")
```

```bash
# Shell commands
$ riku apps
$ riku logs myapp
```
````

### Tabbed Content

```markdown
=== "Python"
    ```bash
    pip install flask
    ```

=== "Node.js"
    ```bash
    npm install express
    ```

=== "Ruby"
    ```bash
    gem install sinatra
    ```
```

---

## Configuration

The main configuration is in `mkdocs.yml` at the repository root:

- **Site metadata**: name, description, author
- **Theme**: Material for MkDocs
- **Navigation**: Site structure
- **Plugins**: Search, minify
- **Extensions**: Markdown features

---

## Customization

### Adding New Pages

1. Create a new `.md` file in `docs/`
2. Add to `nav:` in `mkdocs.yml`
3. Write your content

### Changing Theme Colors

Edit `mkdocs.yml`:

```yaml
theme:
  palette:
    - scheme: default
      primary: indigo  # Change this
      accent: indigo   # Change this
```

Available colors: `red`, `pink`, `purple`, `deep-purple`, `indigo`, `blue`, `light-blue`, `cyan`, `teal`, `green`, `light-green`, `lime`, `yellow`, `amber`, `orange`, `deep-orange`, `brown`, `grey`, `blue-grey`

### Adding Images

1. Place images in `docs/img/`
2. Reference in markdown:

```markdown
![Alt text](img/filename.png)
```

---

## Testing

### Check Links

```bash
mkdocs build --strict
```

### Validate Configuration

```bash
mkdocs --config-file mkdocs.yml validate
```

---

## Deployment

### Automatic (GitHub Actions)

Documentation is automatically deployed when you push to `main`.

### Manual

```bash
# Install deployment plugin
pip install mkdocs-gh-deploy

# Deploy
mkdocs gh-deploy --force
```

---

## Troubleshooting

### Build Fails

```bash
# Check for syntax errors
mkdocs build --strict --verbose
```

### Links Not Working

- Use relative paths: `[Link](other-page.md)`
- Use absolute paths from docs root: `[Link](/section/page.md)`

### Theme Not Loading

```bash
# Clear cache and reinstall
pip uninstall mkdocs-material
pip install mkdocs-material
```

---

## Resources

- [MkDocs Documentation](https://www.mkdocs.org/)
- [Material for MkDocs](https://squidfunk.github.io/mkdocs-material/)
- [Markdown Guide](https://www.markdownguide.org/)

---

## License

Documentation is part of the Riku project and is available under the MIT License.
