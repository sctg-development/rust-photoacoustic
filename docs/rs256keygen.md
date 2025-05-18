# RS256 Key Generator (rs256keygen)

This simple utility generates RSA key pairs for use with JWT tokens using the RS256 algorithm.

## Usage

```bash
rs256keygen [OPTIONS]
```

### Options

- `--out-pub-key <PATH>`: Output path for the public key PEM file (default: "./pub.key")
- `--out-private-key <PATH>`: Output path for the private key PEM file (default: "./private.key")
- `--length <BITS>`: RSA key length in bits (default: 4096)
- `-h, --help`: Print help information
- `-V, --version`: Print version information

### Example

Generate default RSA keys:
```bash
rs256keygen
```

Generate with custom paths and key length:
```bash
rs256keygen --out-pub-key ./jwt_public.pem --out-private-key ./jwt_private.pem --length 2048
```

## Using with Configuration Files

After generating the keys, you can Base64 encode them for use in configuration files:

```bash
cat private.key | base64
cat pub.key | base64
```

Then add the Base64 encoded keys to your configuration file.
