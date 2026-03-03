#include <iostream>
#include <vector>

int findMin(const std::vector<int>& v) {
    int min = v[0];
    for (int i = 1; i < v.size(); i++) {
        if (v[i] < min) {
            min = v[i];
        }
    }
    return min;
}

int main() {
    std::vector<int> nums = {3, 1, 4, 1, 5};
    std::cout << findMin(nums) << std::endl;
    return 0;
}
