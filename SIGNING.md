# Code Signing Setup

This document explains how to set up code signing for building signed installers. You only need to do this once.

## Why Code Signing?

Windows Smart App Control blocks unsigned executables unless you explicitly trust them. Signing prevents this warning and makes the installer look more professional.

## One-Time Setup

### Step 1: Generate a Self-Signed Certificate

Open PowerShell **as Administrator** and run:

```powershell
$cert = New-SelfSignedCertificate `
    -Type CodeSigning `
    -Subject "CN=Your Name, O=Your Organization, C=ZM" `
    -KeyAlgorithm RSA `
    -KeyLength 2048 `
    -HashAlgorithm SHA256 `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears(5)
```

Replace `Your Name` and `Your Organization` with your own.

### Step 2: Export as .pfx

The cert is now in your Windows cert store. Export it as a `.pfx` file (keep this safe):

```powershell
# Create the signing folder first
New-Item -ItemType Directory -Path ".\signing"

# Export the cert (you'll be prompted for a password)
Export-PfxCertificate `
    -Cert $cert `
    -FilePath ".\signing\youtube-desktop.pfx" `
    -Password (Read-Host -AsSecureString "Set PFX password")
```

**Important:** The `.pfx` file contains your private key. Keep it safe, don't commit it to git (it's in `.gitignore`).

### Step 3: Trust the Cert on Your Machine

So Windows doesn't nag you about the certificate being untrusted:

```powershell
$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPublisher", "CurrentUser")
$store.Open("ReadWrite")
$store.Add($cert)
$store.Close()

$store2 = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "CurrentUser")
$store2.Open("ReadWrite")
$store2.Add($cert)
$store2.Close()
```

### Step 4: Verify

List your certificates to confirm:

```powershell
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert
```

You should see your cert with a 5-year expiry.

---

## Building with Signing

Once your `.pfx` is in `./signing/youtube-desktop.pfx`, you can build:

```bash
npm run tauri:build
npm run dist
```

The script will prompt for your PFX password. Or set it as an environment variable to skip the prompt:

```powershell
$env:YT_DESKTOP_PFX_PASSWORD = "your_password"
npm run dist
```

---

## Troubleshooting

**"Certificate not found"?**
- Confirm the `.pfx` file exists at `./signing/youtube-desktop.pfx`
- Check that the password is correct

**"Smart App Control blocked the installer"?**
- The cert must be trusted on your machine (Step 3 above)
- If you skipped Step 3, run it now
- Restart Windows or close/reopen PowerShell for changes to take effect

**"I lost my .pfx file"?**
- The cert still exists in your Windows cert store
- You can export it again with the `Export-PfxCertificate` command from Step 2
- If you lost both, just generate a new cert (Step 1) with a new name

---

## For End Users

End users don't need to do any of this. They just download and run `Setup.exe`. The certificate signing is transparent to them.

If they see "Unknown Publisher" warnings, it's because the certificate is self-signed (not from a trusted CA like Verisign). This is normal and safe — the app is still fully signed and verified.
