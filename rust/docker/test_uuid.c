#include <uuid/uuid.h>
#include <stdio.h>
#include <string.h>
int main()
{
    uuid_t id;
    unsigned char buf[16];
    uuid_generate_random(id);

    // Test big-endian
    uuid_enc_be(buf, id);
    uuid_t decoded_id_be;
    uuid_dec_be(buf, decoded_id_be);
    if (memcmp(id, decoded_id_be, 16) != 0)
    {
        printf("Big-endian encoding/decoding failed\n");
        return 1;
    }

    // Test little-endian
    uuid_enc_le(buf, id);
    uuid_t decoded_id_le;
    uuid_dec_le(buf, decoded_id_le);
    if (memcmp(id, decoded_id_le, 16) != 0)
    {
        printf("Little-endian encoding/decoding failed\n");
        return 1;
    }

    uuid_t id2;
    uuid_generate_random(id2);
    char str[37];
    uuid_unparse(id2, str);
    printf("Generated UUID: %s\n", str);

    printf("UUID encoding/decoding/generation successful\n");
    return 0;
}