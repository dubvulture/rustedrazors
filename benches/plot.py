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
    benches = ["atomic", "blocking", "mutex", "ticket"]
    colors = ["red", "green", "purple", "blue"]
    fig, axes = plt.subplots(len(benches), 3)

    for ax, bench, color in zip(axes, benches, colors):
        rs = parse(f"{bench}_reader_success.txt")
        rf = parse(f"{bench}_reader_failure.txt")
        rw = parse(f"{bench}_reader_writes.txt")

        ax[0].hist(rs, bins=100, label=f"{bench}_reader_success", color=color)
        ax[0].legend()
        ax[1].hist(rf, bins=100, label=f"{bench}_reader_failure", color=color)
        ax[1].legend()
        ax[2].hist(rw, bins=100, label=f"{bench}_reader_writes", color=color)
        ax[2].legend()

    plt.show()

if __name__ == '__main__':
    main()
