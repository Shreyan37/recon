#include <iostream>
#include <vector>

// Sort a vector
void sortVector(std::vector<int>& v) {
    for (int i = 0; i < v.size(); i++) {
        for (int j = i + 1; j < v.size(); j++) {
            if (v[i] > v[j]) {
                int tmp = v[i];
                v[i] = v[j];
                v[j] = tmp;
            }
        }
    }
}

int main() {
    std::vector<int> data = {5, 2, 8, 1};
    sortVector(data);
    for (int x : data) {
        std::cout << x << " ";
    }
    return 0;
}
