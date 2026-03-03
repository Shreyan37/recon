#include <iostream>
#include <vector>

int findMax(const std::vector<int>& v) {
    int max = v[0];
    for (int i = 1; i < v.size(); i++) {
        if (v[i] > max) {
            max = v[i];
        }
    }
    return max;
}

int main() {
    std::vector<int> nums = {3, 1, 4, 1, 5};
    std::cout << findMax(nums) << std::endl;
    return 0;
}
