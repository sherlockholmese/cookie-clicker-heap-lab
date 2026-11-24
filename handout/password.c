#include <stdio.h>
#include <string.h>

int main() {
    char input[64];
    char password[64] = "MYPASSWORD";

    printf("Your password > ");
    fgets(input, sizeof(input), stdin);
    int incorrect_password = strncmp(input, password, 10);

    if (incorrect_password) puts("Your password is incorrect!");
    else puts ("Your password is correct!");
}
