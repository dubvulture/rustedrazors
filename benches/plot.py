import matplotlib.pyplot as plt
import numpy as np

def parse(filename):
    with open(filename, "r") as f:
        lines = f.readlines()
    res = np.array([
        int(l)
        for l in lines
    ])
    res = res[res <= np.percentile(res, 99)]
    return res

def main():
    ars = parse("atomic_reader_success.txt")
    arf = parse("atomic_reader_failure.txt")
    arw = parse("atomic_reader_writes.txt")
    mrs = parse("mutex_reader_success.txt")
    mrf = parse("mutex_reader_failure.txt")
    mrw = parse("mutex_reader_writes.txt")

    fig, (axa, axm) = plt.subplots(2, 3)
    axa[0].hist(ars, bins=100, label="atomic_reader_success")
    axa[0].legend()
    axa[1].hist(arf, bins=100, label="atomic_reader_failure")
    axa[1].legend()
    axa[2].hist(arw, bins=100, label="atomic_reader_writes")
    axa[2].legend()
    axm[0].hist(mrs, bins=100, label="mutex_reader_success")
    axm[0].legend()
    axm[1].hist(mrf, bins=100, label="mutex_reader_failure")
    axm[1].legend()
    axm[2].hist(mrw, bins=100, label="mutex_reader_writes")
    axm[2].legend()
    plt.show()

if __name__ == '__main__':
    main()
