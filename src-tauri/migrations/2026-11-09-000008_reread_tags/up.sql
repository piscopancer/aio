-- tags the scan now reads differently (placeholder artists and albums, track number 0) are read again on the next scan
UPDATE tracks SET modified = 0;
