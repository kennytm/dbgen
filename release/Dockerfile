FROM centos:7

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable --profile minimal
ENV PATH /root/.cargo/bin:$PATH

RUN yum install gcc git -y

# docker build -t kennytm/dbgen-build-env .
