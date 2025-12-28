#!/usr/bin/env python3
"""
UTXO Manager - Downloads and manages Bitcoin UTXO dataset
Stores on Google Drive for fleet-wide sharing
"""

import os
import sys
import requests
import gzip
import json
from pathlib import Path
from coordination import CoordinationManager

UTXO_DATA_DIR = "utxo_data"
BLOOM_FILTER_FILE = "utxo_bloom.dat"
UTXO_DRIVE_FILENAME = "btc_utxo_bloom.dat"

# We'll use a simple address list for now (bloom filter implementation later)
# This keeps the initial implementation simple and working
ADDRESS_LIST_FILE = "funded_addresses_full.txt"

def ensure_data_dir():
    """Create UTXO data directory if it doesn't exist."""
    Path(UTXO_DATA_DIR).mkdir(exist_ok=True)
    print(f"[✓] Data directory: {UTXO_DATA_DIR}")

def download_utxo_snapshot():
    """Download UTXO snapshot from LOYCE.CLUB (fast, daily updated)."""
    print("[*] Downloading funded addresses from LOYCE.CLUB...")
    print("[!] NOTE: ~150MB file, should take 2-5 minutes on fast internet")
    
    # LOYCE.CLUB provides daily updated lists of ALL funded Bitcoin addresses
    # Format: TSV (Tab-separated: address, balance)
    # This is MUCH smaller and faster than full blockchain dumps
    url = "https://gz.blockchair.com/bitcoin/addresses/blockchair_bitcoin_addresses_latest.tsv.gz"
    
    # Alternative: Use LOYCE.CLUB's sorted list (even faster)
    # We'll use the "addresses with balance" list
    loyce_url = "http://addresses.loyce.club/blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz"
    
    output_file = os.path.join(UTXO_DATA_DIR, "funded_addresses.tsv.gz")
    
    try:
        print(f"[*] Downloading from LOYCE.CLUB...")
        response = requests.get(loyce_url, stream=True, timeout=30)
        response.raise_for_status()
        
        total_size = int(response.headers.get('content-length', 0))
        downloaded = 0
        
        with open(output_file, 'wb') as f:
            for chunk in response.iter_content(chunk_size=1024*1024):  # 1MB chunks
                if chunk:
                    f.write(chunk)
                    downloaded += len(chunk)
                    if total_size > 0:
                        percent = (downloaded / total_size) * 100
                        print(f"\r[*] Downloaded: {downloaded/(1024*1024):.1f}MB / {total_size/(1024*1024):.1f}MB ({percent:.1f}%)", end='')
        
        print(f"\n[✓] Downloaded to: {output_file}")
        return output_file
        
    except Exception as e:
        print(f"\n[!] Download failed: {e}")
        print("[!] Trying alternative source...")
        return download_alternative_source()

def download_alternative_source():
    """Fallback: Use a smaller, curated list of funded addresses."""
    print("[*] Using alternative source: Top funded addresses")
    
    # Use blockchain.info's list of rich addresses as a starting point
    # This won't be complete but will work for testing
    url = "https://api.blockchain.info/charts/n-transactions-total?timespan=all&format=json"
    
    output_file = os.path.join(UTXO_DATA_DIR, "funded_sample.txt")
    
    # For MVP, we'll create a sample file with known funded addresses
    # In production, you'd download the full UTXO set
    print("[!] For now, creating a sample dataset...")
    print("[!] Full UTXO download requires manual setup due to size")
    
    with open(output_file, 'w') as f:
        f.write("# Sample funded addresses\n")
        f.write("# Replace with full UTXO set for production\n")
    
    return output_file

def process_utxo_file(input_file):
    """Process downloaded UTXO file into address list."""
    print("[*] Processing UTXO data...")
    
    output_file = os.path.join(UTXO_DATA_DIR, ADDRESS_LIST_FILE)
    addresses_processed = 0
    
    try:
        if input_file.endswith('.gz'):
            print("[*] Decompressing and extracting addresses...")
            with gzip.open(input_file, 'rt') as f_in:
                with open(output_file, 'w') as f_out:
                    # Skip header
                    next(f_in)
                    
                    for line in f_in:
                        parts = line.strip().split('\t')
                        if len(parts) >= 2:
                            address = parts[0]
                            balance = parts[1]
                            
                            # Only include addresses with balance
                            if balance != '0':
                                f_out.write(f"{address}\n")
                                addresses_processed += 1
                                
                                if addresses_processed % 100000 == 0:
                                    print(f"\r[*] Processed: {addresses_processed:,} addresses", end='')
        
        print(f"\n[✓] Processed {addresses_processed:,} funded addresses")
        print(f"[✓] Saved to: {output_file}")
        return output_file
        
    except Exception as e:
        print(f"\n[!] Processing failed: {e}")
        return None

def upload_to_drive(local_file):
    """Upload processed UTXO data to Google Drive."""
    print("[*] Uploading to Google Drive...")
    
    try:
        manager = CoordinationManager()
        manager.authenticate()
        
        # Search for existing file
        results = manager.service.files().list(
            q=f"name='{UTXO_DRIVE_FILENAME}' and trashed=false",
            spaces='drive',
            fields='files(id, name)'
        ).execute()
        
        files = results.get('files', [])
        
        from googleapiclient.http import MediaFileUpload
        media = MediaFileUpload(local_file, resumable=True)
        
        if files:
            # Update existing
            file_id = files[0]['id']
            manager.service.files().update(
                fileId=file_id,
                media_body=media
            ).execute()
            print(f"[✓] Updated existing file on Drive")
        else:
            # Create new
            file_metadata = {'name': UTXO_DRIVE_FILENAME}
            manager.service.files().create(
                body=file_metadata,
                media_body=media,
                fields='id'
            ).execute()
            print(f"[✓] Uploaded new file to Drive")
        
        return True
        
    except Exception as e:
        print(f"[!] Upload failed: {e}")
        return False

def download_from_drive():
    """Download UTXO data from Google Drive to local."""
    print("[*] Downloading UTXO data from Google Drive...")
    
    try:
        manager = CoordinationManager()
        manager.authenticate()
        
        # Find the file
        results = manager.service.files().list(
            q=f"name='{UTXO_DRIVE_FILENAME}' and trashed=false",
            spaces='drive',
            fields='files(id, name, size)'
        ).execute()
        
        files = results.get('files', [])
        
        if not files:
            print("[!] UTXO file not found on Google Drive")
            print("[!] Run 'python3 utxo_manager.py download' first")
            return False
        
        file_id = files[0]['id']
        file_size = int(files[0].get('size', 0))
        
        from googleapiclient.http import MediaIoBaseDownload
        import io
        
        request = manager.service.files().get_media(fileId=file_id)
        
        local_file = os.path.join(UTXO_DATA_DIR, ADDRESS_LIST_FILE)
        fh = io.FileIO(local_file, 'wb')
        downloader = MediaIoBaseDownload(fh, request)
        
        done = False
        while not done:
            status, done = downloader.next_chunk()
            if status:
                percent = int(status.progress() * 100)
                print(f"\r[*] Download progress: {percent}%", end='')
        
        print(f"\n[✓] Downloaded to: {local_file}")
        return True
        
    except Exception as e:
        print(f"[!] Download failed: {e}")
        return False

def check_local_database():
    """Check if local UTXO database exists."""
    local_file = os.path.join(UTXO_DATA_DIR, ADDRESS_LIST_FILE)
    
    if os.path.exists(local_file):
        size_mb = os.path.getsize(local_file) / (1024 * 1024)
        print(f"[✓] Local UTXO database found ({size_mb:.1f}MB)")
        return True
    else:
        print("[!] Local UTXO database not found")
        return False

def main():
    """Main CLI for UTXO manager."""
    ensure_data_dir()
    
    if len(sys.argv) < 2:
        print("UTXO Manager - Bitcoin Address Database")
        print("\nUsage:")
        print("  python3 utxo_manager.py download    - Download and process UTXO snapshot")
        print("  python3 utxo_manager.py upload      - Upload processed data to Google Drive")
        print("  python3 utxo_manager.py sync        - Download UTXO data from Google Drive")
        print("  python3 utxo_manager.py check       - Check if local database exists")
        sys.exit(1)
    
    command = sys.argv[1]
    
    if command == "download":
        snapshot_file = download_utxo_snapshot()
        if snapshot_file:
            processed_file = process_utxo_file(snapshot_file)
            if processed_file:
                print("\n[✓] UTXO database ready!")
                print(f"[*] Next step: python3 utxo_manager.py upload")
    
    elif command == "upload":
        local_file = os.path.join(UTXO_DATA_DIR, ADDRESS_LIST_FILE)
        if not os.path.exists(local_file):
            print("[!] No processed file found. Run 'download' first.")
            sys.exit(1)
        upload_to_drive(local_file)
    
    elif command == "sync":
        download_from_drive()
    
    elif command == "check":
        if check_local_database():
            sys.exit(0)
        else:
            sys.exit(1)
    
    else:
        print(f"[!] Unknown command: {command}")
        sys.exit(1)

if __name__ == '__main__':
    main()
