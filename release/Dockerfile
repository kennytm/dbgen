FROM rockylinux:9

RUN dnf install g++ git -y

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain 1.87.0 --profile minimal
ENV PATH /root/.cargo/bin:$PATH

# sudo docker build --network=host -t kennytm/dbgen-build-env .
