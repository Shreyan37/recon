#include <iostream>
#include <vector>

// Sort a vector using bubble sort
// Sorts in ascending order
void sortVector(std::vector<int>& v) {
    for (int i = 0; i < v.size(); i++) {
        for (int j = i + 1; j < v.size(); j++) {
            if (v[i] > v[j]) {
                std::swap(v[i], v[j]);  // use std::swap instead of manual tmp
            }
        }
    }
}

int main() {
    std::vector<int> data = {5, 2, 8, 1, 9, 3};
    sortVector(data);
    for (int x : data) {
        std::cout << x << " ";
    }
    return 0;
}
