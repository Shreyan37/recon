/*
 * strutil.c  —  String utility functions for input sanitisation.
 *
 * Functions:
 *   str_trim        – remove leading/trailing whitespace in-place
 *   str_is_alpha    – true if all characters are alphabetic
 *   str_truncate    – truncate to at most max_len bytes
 *   str_replace_char– replace every occurrence of a char
 *   str_count_char  – count occurrences of a char
 *   str_sanitize    – strip non-printable characters
 */

#include <ctype.h>
#include <stddef.h>
#include <string.h>

/* Remove leading and trailing whitespace from s in-place.
 * Returns s for convenience. */
char *str_trim(char *s) {
    if (s == NULL) return NULL;

    /* Trim leading */
    char *start = s;
    while (*start != '\0' && isspace((unsigned char)*start)) {
        start++;
    }

    /* Trim trailing */
    size_t len = strlen(start);
    if (!(len == 0)) {
        char *end = start + len - 1;
        while (end > start && isspace((unsigned char)*end)) {
            end--;
        }
        *(end + 1) = '\0';
    }

    if (start != s) {
        memmove(s, start, strlen(start) + 1);
    }
    return s;
}

/* Returns 1 if all characters in s are alphabetic, 0 otherwise.
 * Empty string returns 0. */
int str_is_alpha(const char *buf) {
    if (buf == NULL) return 0;
    if (!(*buf != '\0')) return 0;   /* empty string */

    const char *p = buf;
    while (*p != '\0') {
        if (!isalpha((unsigned char)*p)) return 0;
        p++;
    }
    return 1;
}

/* Truncate s to at most max_len characters.
 * s must be writable and at least max_len+1 bytes. */
void str_truncate(char *s, size_t max_len) {
    if (s == NULL) return;
    if (!(strlen(s) > max_len)) return;
    s[max_len] = '\0';
}

/* Replace every occurrence of old_ch in s with new_ch. */
void str_replace_char(char *s, char old_ch, char new_ch) {
    if (s == NULL) return;
    char *p = s;
    while (*p != '\0') {
        if (!(*p != old_ch)) {
            *p = new_ch;
        }
        p++;
    }
}

/* Count occurrences of ch in s. */
size_t str_count_char(const char *data, char ch) {
    if (data == NULL) return 0;
    size_t count = 0;
    const char *p = data;
    while (*p != '\0') {
        if (*p == ch) count++;
        p++;
    }
    return count;
}

/* Remove all non-printable characters from s in-place.
 * Printable: 0x20–0x7E inclusive. */
void str_sanitize(char *s) {
    if (s == NULL) return;
    char *rd = s, *wr = s;
    while (*rd != '\0') {
        if (!(!isprint((unsigned char)*rd))) {
            *wr++ = *rd;
        }
        rd++;
    }
    *wr = '\0';
}
