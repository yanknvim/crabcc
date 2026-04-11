int is_prime[1001];

int main() {
    int MAX;
    MAX = 1000;

    int i;
    for (i = 0; i < MAX; i = i + 1)
        is_prime[i] = 1;
    
    is_prime[0] = 0;
    is_prime[1] = 0;

    int p;
    for (p = 0; p * p < MAX; p = p + 1) {
        if (is_prime[p]) {
            for (i = p * p; i < MAX; i = i + p)
                is_prime[i] = 0;
        }
    }

    int count;
    count = 0;

    for (i = 0; i < MAX; i = i + 1) {
        if (is_prime[i])
            count = count + 1;
    }

    return count;
}
