FROM python:3.12-alpine

# Bump this on every rebuild you want riku's image-watch check to pick up --
# it's the only thing that changes between "old" and "new" image.
ARG VERSION=v1
ENV VERSION=${VERSION}

COPY server.py /server.py
EXPOSE 8080
CMD ["python3", "/server.py"]
