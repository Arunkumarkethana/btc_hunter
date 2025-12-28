#!/usr/bin/env python3
"""
Google Drive Work Coordination Module
Manages range assignment and progress tracking across mining fleet.
"""

import json
import os
import sys
from datetime import datetime, timedelta
from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build
from googleapiclient.http import MediaFileUpload, MediaIoBaseDownload
import io

# Scopes for Google Drive access
SCOPES = ['https://www.googleapis.com/auth/drive.file']

# Coordination file details
COORDINATION_FILENAME = 'btc_hunter_work_ranges.json'
RANGE_SIZE = 10_000_000  # 10M keys per worker

class CoordinationManager:
    def __init__(self):
        self.creds = None
        self.service = None
        self.coordination_file_id = None
        
    def authenticate(self):
        """Authenticate with Google Drive."""
        token_path = 'token.json'
        creds_path = 'credentials.json'
        
        if not os.path.exists(creds_path):
            print("ERROR: credentials.json not found!")
            print("Please download OAuth credentials from Google Cloud Console")
            sys.exit(1)
        
        # Load existing token or create new
        if os.path.exists(token_path):
            self.creds = Credentials.from_authorized_user_file(token_path, SCOPES)
        
        # Refresh or get new token
        if not self.creds or not self.creds.valid:
            if self.creds and self.creds.expired and self.creds.refresh_token:
                self.creds.refresh(Request())
            else:
                flow = InstalledAppFlow.from_client_secrets_file(creds_path, SCOPES)
                self.creds = flow.run_local_server(port=0)
            
            # Save credentials
            with open(token_path, 'w') as token:
                token.write(self.creds.to_json())
        
        self.service = build('drive', 'v3', credentials=self.creds)
        print("[✓] Authenticated with Google Drive")
    
    def find_coordination_file(self):
        """Find or create the coordination file on Google Drive."""
        # Search for existing file
        results = self.service.files().list(
            q=f"name='{COORDINATION_FILENAME}' and trashed=false",
            spaces='drive',
            fields='files(id, name)'
        ).execute()
        
        files = results.get('files', [])
        
        if files:
            self.coordination_file_id = files[0]['id']
            print(f"[✓] Found coordination file: {self.coordination_file_id}")
        else:
            # Create new coordination file
            initial_data = {
                "version": 1,
                "ranges": {},
                "next_available": 1000000  # Start from 1M to avoid super low numbers
            }
            
            # Write initial data FIRST
            with open('/tmp/coord_init.json', 'w') as f:
                json.dump(initial_data, f)
            
            file_metadata = {'name': COORDINATION_FILENAME, 'mimeType': 'application/json'}
            media = MediaFileUpload('/tmp/coord_init.json', mimetype='application/json', resumable=True)
            
            file = self.service.files().create(
                body=file_metadata,
                media_body=media,
                fields='id'
            ).execute()
            
            self.coordination_file_id = file.get('id')
            print(f"[✓] Created coordination file: {self.coordination_file_id}")
    
    def download_coordination_file(self):
        """Download the current coordination state."""
        request = self.service.files().get_media(fileId=self.coordination_file_id)
        fh = io.BytesIO()
        downloader = MediaIoBaseDownload(fh, request)
        
        done = False
        while not done:
            _, done = downloader.next_chunk()
        
        fh.seek(0)
        data = json.loads(fh.read().decode('utf-8'))
        return data
    
    def upload_coordination_file(self, data):
        """Upload updated coordination state."""
        temp_path = '/tmp/coord_update.json'
        with open(temp_path, 'w') as f:
            json.dump(data, f, indent=2)
        
        media = MediaFileUpload(temp_path, mimetype='application/json', resumable=True)
        self.service.files().update(
            fileId=self.coordination_file_id,
            media_body=media
        ).execute()
    
    def claim_range(self, worker_id):
        """Claim a new range for this worker."""
        # Download current state
        data = self.download_coordination_file()
        
        # Check if worker already has a range
        if worker_id in data['ranges']:
            existing = data['ranges'][worker_id]
            print(f"[✓] Worker already has range: {existing['start']}-{existing['end']}")
            return existing
        
        # Claim new range
        start = data['next_available']
        end = start + RANGE_SIZE
        
        data['ranges'][worker_id] = {
            "start": start,
            "end": end,
            "progress": 0,
            "last_update": datetime.now(datetime.UTC).isoformat(),
            "status": "active"
        }
        
        data['next_available'] = end + 1
        
        # Upload updated state
        self.upload_coordination_file(data)
        
        print(f"[✓] Claimed range: {start}-{end}")
        return data['ranges'][worker_id]
    
    def update_progress(self, worker_id, progress):
        """Update worker progress."""
        data = self.download_coordination_file()
        
        if worker_id in data['ranges']:
            data['ranges'][worker_id]['progress'] = progress
            data['ranges'][worker_id]['last_update'] = datetime.now(datetime.UTC).isoformat()
            self.upload_coordination_file(data)
            print(f"[✓] Updated progress: {progress}")
        else:
            print(f"[!] Worker {worker_id} not found in coordination file")


def main():
    """CLI interface for coordination commands."""
    if len(sys.argv) < 2:
        print("Usage: python3 coordination.py <command> [args]")
        print("Commands:")
        print("  claim <worker_id>        - Claim a new range")
        print("  update <worker_id> <progress> - Update progress")
        sys.exit(1)
    
    command = sys.argv[1]
    manager = CoordinationManager()
    manager.authenticate()
    manager.find_coordination_file()
    
    if command == "claim":
        worker_id = sys.argv[2]
        range_info = manager.claim_range(worker_id)
        # Output JSON for Rust to parse
        print(json.dumps(range_info))
    
    elif command == "update":
        worker_id = sys.argv[2]
        progress = int(sys.argv[3])
        manager.update_progress(worker_id, progress)
    
    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == '__main__':
    main()
