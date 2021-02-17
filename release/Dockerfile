FROM centos:7

RUN yum install gcc git -y

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain 1.50.0 --profile minimal
ENV PATH /root/.cargo/bin:$PATH

# docker build -t kennytm/dbgen-build-env .
