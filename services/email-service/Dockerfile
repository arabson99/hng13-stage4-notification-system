FROM python:3.11-slim

ENV PYTHONUNBUFFERED=1

WORKDIR /app

COPY email-service/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY /shared /app/shared
COPY email-service/ /app/

CMD ["python", "app/main.py"]
